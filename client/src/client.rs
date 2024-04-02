use std::rc::Rc;
use std::sync::Mutex;

use anyhow::anyhow;
use bincode::{deserialize, serialize};
use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};

use crate::bridge::{flog, log, log_js_value, timestamp_ms, websocket_url};
use crate::scene::comms::{ClientEvent, ClientMessage, ServerEvent};

pub struct Client {
    sock: Sock,

    // Count frames and ping server every so often if no events have been sent,
    // to check game is still live.
    counter: u32,

    // Key of game this client is connected to.
    game_key: String,
}

/// The `Client` handles sending `ClientMessage`s to the server and receiving
/// `ServerMessage`s form the server. It opens a `WebSocket` with the server
/// and listens on this socket, posting messages if `send_message` is used.
impl Client {
    const PING_INTERVAL_FRAMES: u32 = 300; // For 60fps screen, every 5s.

    /// If the page URL is /game/GAME_KEY/client/CLIENT_KEY, this will attempt
    /// to connect to the appropriate game websocket. If the URL doesn't match
    /// will return Ok(None). On successfully connection returns
    /// Ok(Some(Client)) on a failed connection returns Err.
    pub fn new() -> anyhow::Result<Option<Client>> {
        let (url, game_key) = match websocket_url() {
            Ok(Some(val)) => val,
            _ => return Ok(None),
        };

        let incoming_events = Rc::new(Mutex::new(Vec::new()));
        let sock = Sock::new(url, incoming_events)?;

        Ok(Some(Client {
            sock,
            counter: 0,
            game_key,
        }))
    }

    // Returns vector of events ordered from newest to oldest.
    // This order is chosen because it allows popping from the end in order to
    // apply events in the correct order.
    pub fn events(&mut self) -> Vec<ServerEvent> {
        self.counter += 1;
        if self.counter >= Self::PING_INTERVAL_FRAMES {
            self.ping();
        }

        if !self.sock.health_check() {
            self.disconnected_redirect();
        }
        self.sock.load_events()
    }

    pub fn disconnect(&mut self) {
        log("Disconnected due to inactivity.");
        self.sock.disconnect();
        self.disconnected_redirect();
    }

    fn disconnected_redirect(&self) {
        const HREF: &str = "/disconnected";
        crate::bridge::redirect_to(&format!("{HREF}?from={}", self.game_key))
    }

    pub fn send_message(&mut self, message: &ClientMessage) {
        // Reset counter every time a message is sent.
        self.counter = 0;
        self.sock.send_message(message, true);
    }

    fn ping(&mut self) {
        self.send_message(&ClientMessage {
            id: 0,
            event: ClientEvent::Ping,
        });
    }
}

type SockRef = Rc<Mutex<WebSocket>>;
type EventsRef = Rc<Mutex<Vec<ServerEvent>>>;

// https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/readyState
#[derive(Debug, Eq, PartialEq)]
enum ReadyState {
    Unknown = -1,
    Connecting = 0,
    Open = 1,
    Closing = 2,
    Closed = 3,
}

impl ReadyState {
    fn matches(self, ready_state: u16) -> bool {
        self as u16 == ready_state
    }
}

struct Sock {
    url: String,
    socket: WebSocket,
    events: EventsRef,
    reconnect_attempts: u32,
    last_connected: u64,
    last_reconnect_attempt: u64,
    terminated: bool,
}

impl Sock {
    const BACKOFF_COEFFICIENT: u32 = 2;
    const RECONNECT_MAX_DURATION_MS: u64 = 60 * 1000;

    fn new(url: String, events: EventsRef) -> anyhow::Result<Self> {
        let socket = create_websocket(&url, events.clone())?;
        Ok(Self {
            url,
            socket,
            events,
            reconnect_attempts: 0,
            last_connected: timestamp_ms(),
            last_reconnect_attempt: 0,
            terminated: false,
        })
    }

    fn load_events(&self) -> Vec<ServerEvent> {
        match self.events.try_lock() {
            Ok(mut events) => std::mem::take(&mut *events),
            Err(_) => {
                log("Failed to lock socket events.");
                Vec::new()
            }
        }
    }

    fn send_message(&self, message: &ClientMessage, retry: bool) {
        if self.ready_state() != ReadyState::Open {
            flog!(
                "Not sending message as socket state is {:?}.",
                self.ready_state()
            );
            return;
        }

        if let Ok(data) = serialize(message) {
            if let Err(v) = self.socket.send_with_u8_array(&data) {
                log("Failed to send event. Reason:");
                log_js_value(&v);
                if retry {
                    log("Retrying send event.");
                    self.send_message(message, false);
                }
            }
        } else {
            log("Failed to serialise message to send.");
        }
    }

    fn health_check(&mut self) -> bool {
        if self.terminated {
            return false;
        }

        let now_ms = timestamp_ms();

        let ready_state = self.ready_state();
        match ready_state {
            ReadyState::Open => {
                self.last_connected = now_ms;
            }
            ReadyState::Closed => {
                let since_last_reconnect = now_ms.saturating_sub(self.last_reconnect_attempt);
                if Self::BACKOFF_COEFFICIENT.pow(self.reconnect_attempts) as u64
                    <= since_last_reconnect
                {
                    self.connect();
                }
            }
            _ => {}
        }

        // If we failed to connect / reconnect for max duration, give up.
        if ready_state != ReadyState::Open {
            let closed_for = now_ms.saturating_sub(self.last_connected);
            if closed_for > Self::RECONNECT_MAX_DURATION_MS {
                self.terminated = true;
            }
        }

        !self.terminated
    }

    fn ready_state(&self) -> ReadyState {
        match self.socket.ready_state() {
            0 => ReadyState::Connecting,
            1 => ReadyState::Open,
            2 => ReadyState::Closing,
            3 => ReadyState::Closed,
            _ => ReadyState::Unknown,
        }
    }

    fn connect(&mut self) {
        if self.terminated {
            return;
        }

        self.reconnect_attempts += 1;
        self.last_reconnect_attempt = timestamp_ms();
        flog!(
            "Reconnecting websocket (attempt {})",
            self.reconnect_attempts
        );

        if let Ok(socket) = create_websocket(&self.url, self.events.clone()) {
            // Close existing socket.
            self.socket.close().ok();

            // Set new socket.
            self.socket = socket;
        }
    }

    fn disconnect(&mut self) {
        self.socket.close().ok();
        self.terminated = true;
    }
}

fn deserialise_message(message: JsValue) -> anyhow::Result<ServerEvent> {
    match message.dyn_into::<ArrayBuffer>() {
        Ok(b) => Ok(deserialize(&Uint8Array::new(&b).to_vec())?),
        Err(e) => Err(anyhow!(
            "WebSocket message could not be cast to ArrayBuffer: {e:?}."
        )),
    }
}

fn create_websocket(url: &str, events: EventsRef) -> anyhow::Result<WebSocket> {
    flog!("Connecting WebSocket.");

    let ws = match WebSocket::new(url) {
        Ok(ws) => ws,
        Err(e) => return Err(anyhow!("Failed to create WebSocket: {e:?}")),
    };

    // More performant than Blob for small payloads, per the wasm-bindgen
    // example at
    // https://rustwasm.github.io/wasm-bindgen/examples/websockets.html
    ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

    let onmessage =
        Closure::wrap(
            Box::new(move |e: MessageEvent| match deserialise_message(e.data()) {
                Ok(event) => match events.try_lock() {
                    Ok(mut lock) => lock.push(event),
                    Err(_) => log("Failed to lock events."),
                },
                Err(s) => flog!("WebSocket decode error: {s}"),
            }) as Box<dyn FnMut(MessageEvent)>,
        );
    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    let onerror = Closure::wrap(Box::new(move |e: ErrorEvent| {
        flog!("Closed due to error: {:?}", e.as_string());
    }) as Box<dyn FnMut(ErrorEvent)>);
    ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    let onclose = Closure::wrap(Box::new(move |e: CloseEvent| {
        flog!("WebSocket closed: {:?} (Code {})", e.as_string(), e.code())
    }) as Box<dyn FnMut(CloseEvent)>);
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();

    Ok(ws)
}
