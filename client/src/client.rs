use std::rc::Rc;

use anyhow::anyhow;
use bincode::{deserialize, serialize};
use js_sys::{ArrayBuffer, Uint8Array};
use parking_lot::Mutex;
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};

use crate::bridge::{flog, game_over_redirect, log, log_js_value, websocket_url};
use crate::scene::comms::{ClientEvent, ClientMessage, ServerEvent};

type Events = Rc<Mutex<Vec<ServerEvent>>>;
type Sock = Rc<Mutex<WebSocket>>;

pub struct Client {
    sock: Sock,
    incoming_events: Events,

    // Count frames and ping server every so often if no events have been sent,
    // to check game is still live.
    counter: u32,
}

/// The `Client` handles sending `ClientMessage`s to the server and receiving
/// `ServerMessage`s form the server. It opens a `WebSocket` with the server
/// and listens on this socket, posting messages if `send_message` is used.
impl Client {
    const PING_INTERVAL_FRAMES: u32 = 60 * 30; // For 60fps screen, every 30s.

    /// If the page URL is /game/GAME_KEY/client/CLIENT_KEY, this will attempt
    /// to connect to the appropriate game websocket. If the URL doesn't match
    /// will return Ok(None). On successfully connection returns
    /// Ok(Some(Client)) on a failed connection returns Err.
    pub fn new() -> anyhow::Result<Option<Client>> {
        let url = match websocket_url() {
            Ok(Some(url)) => url,
            _ => return Ok(None),
        };

        let incoming_events = Rc::new(Mutex::new(Vec::new()));
        let sock = connect_websocket(url, incoming_events.clone())?;

        Ok(Some(Client {
            sock,
            incoming_events,
            counter: 0,
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

        let mut events = self.incoming_events.lock();
        let mut ret = Vec::new();
        ret.append(&mut events);
        ret
    }

    fn _send_message(&self, message: &[u8], retry: bool) {
        if let Err(v) = self.sock.lock().send_with_u8_array(message) {
            if retry {
                self._send_message(message, false);
            } else {
                log("Failed to send event. Reason:");
                log_js_value(&v);
            }
        }
    }

    pub fn send_message(&mut self, message: &ClientMessage) {
        // Reset counter every time a message is sent.
        self.counter = 0;
        if let Ok(m) = serialize(message) {
            self._send_message(&m, true);
        } else {
            log("Failed to serialise message to send.");
        }
    }

    fn ping(&mut self) {
        self.send_message(&ClientMessage {
            id: 0,
            event: ClientEvent::Ping,
        });
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

fn create_websocket(url: &str, events: Events) -> anyhow::Result<WebSocket> {
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
                Ok(e) => events.lock().push(e),
                Err(s) => flog!("WebSocket decode error: {s}"),
            }) as Box<dyn FnMut(MessageEvent)>,
        );
    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    Ok(ws)
}

fn reconnect_websocket(url: String, events: Events, sock: Sock) {
    if let Ok(replacement) = create_websocket(&url, events.clone()) {
        *sock.lock() = replacement;
        add_handlers(url, events, sock);
    } else {
        flog!("Failed to reconnect. Redirecting.");
        game_over_redirect();
    }
}

fn add_handlers(url: String, events: Events, sock: Sock) {
    let url_ref = url.clone();
    let sock_ref = sock.clone();
    let events_ref = events.clone();
    let onerror = Closure::wrap(Box::new(move |e: ErrorEvent| {
        flog!("Attempting reconnect. Closed due to error:");
        log_js_value(&e);
        reconnect_websocket(url_ref.clone(), events_ref.clone(), sock_ref.clone());
    }) as Box<dyn FnMut(ErrorEvent)>);
    sock.lock()
        .set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    // Create handler to replace the socket if it closes. If the server closes
    // the socket it will set a reason indicating as such and we will not try
    // to reopen it.
    let sock_ref = sock.clone();
    let onclose = Closure::wrap(Box::new(move |e: CloseEvent| {
        flog!("WebSocket closed.");
        if &e.reason() == "gameover" {
            flog!("Closed due to game over. Redirecting.");
            game_over_redirect();
        } else {
            flog!("Attempting reconnect. Closed due to:");
            log_js_value(&e);
            reconnect_websocket(url.clone(), events.clone(), sock_ref.clone());
        }
    }) as Box<dyn FnMut(CloseEvent)>);
    sock.lock()
        .set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();
}

/// Connects a websocket, returning a Mutex to that websocket. In the event
/// that the socket is closed, this will replace the value in the Mutex with a
/// new socket, setting ready false in the interim.
fn connect_websocket(url: String, events: Events) -> anyhow::Result<Sock> {
    // Mutex on the websocket.
    let sock = Rc::new(Mutex::new(create_websocket(&url, events.clone())?));
    add_handlers(url, events, sock.clone());
    Ok(sock)
}
