use std::sync::atomic::AtomicBool;
use std::{rc::Rc, sync::atomic::Ordering};

use anyhow::anyhow;
use bincode::{deserialize, serialize};
use js_sys::{ArrayBuffer, Uint8Array};
use parking_lot::Mutex;
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{ErrorEvent, MessageEvent, WebSocket};

use crate::bridge::{flog, log, log_js_value, websocket_url};
use crate::scene::comms::{ClientEvent, ClientMessage, ServerEvent};

type Events = Rc<Mutex<Vec<ServerEvent>>>;
type Sock = Rc<Mutex<WebSocket>>;

pub struct Client {
    ready: Rc<AtomicBool>,
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

        let ready = Rc::new(AtomicBool::new(false));
        let incoming_events = Rc::new(Mutex::new(Vec::new()));
        let sock = connect_websocket(url, ready.clone(), incoming_events.clone())?;

        Ok(Some(Client {
            ready,
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
        Ok(b) => match deserialize(&Uint8Array::new(&b).to_vec()) {
            Ok(e) => {
                if matches!(e, ServerEvent::GameOver) {
                    redirect_game_over();
                    Err(anyhow!("Game over."))
                } else {
                    Ok(e)
                }
            }
            Err(e) => Err(anyhow!("WebSocket message deserialisation failed: {e}.")),
        },
        Err(e) => Err(anyhow!(
            "WebSocket message could not be cast to ArrayBuffer: {e:?}."
        )),
    }
}

fn redirect_game_over() {
    web_sys::window()
        .expect("Missing window.")
        .location()
        .set_href("/game_over")
        .ok();
}

fn create_websocket(url: &str, ready: Rc<AtomicBool>, events: Events) -> anyhow::Result<WebSocket> {
    ready.store(false, Ordering::Relaxed);
    crate::bridge::flog!("Connecting WebSocket.");

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

    let onerror = Closure::wrap(Box::new(move |e: ErrorEvent| {
        crate::bridge::flog!("WebSocket error: {e:?}");
        redirect_game_over();
    }) as Box<dyn FnMut(ErrorEvent)>);
    ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    let onopen = Closure::wrap(Box::new(move |_| {
        crate::bridge::flog!("WebSocket connected.");
        ready.store(true, Ordering::Relaxed)
    }) as Box<dyn FnMut(JsValue)>);
    ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    Ok(ws)
}

/// Connects a websocket, returning a Mutex to that websocket. In the event
/// that the socket is closed, this will replace the value in the Mutex with a
/// new socket, setting ready false in the interim.
fn connect_websocket(url: String, ready: Rc<AtomicBool>, events: Events) -> anyhow::Result<Sock> {
    // Mutex on the websocket.
    let sock = Rc::new(Mutex::new(create_websocket(
        &url,
        ready.clone(),
        events.clone(),
    )?));

    // Create handler to replace the socket if it closes.
    let sock_ref = sock.clone();
    let onclose = Closure::wrap(Box::new(move |_| {
        crate::bridge::flog!("WebSocket closed. Attemting reconnect.");
        if let Ok(replacement) = create_websocket(&url, ready.clone(), events.clone()) {
            let mut lock = sock_ref.lock();
            *lock = replacement;
        } else {
            crate::bridge::flog!("Failed to reconnect. Game over.");
            redirect_game_over();
        }
    }) as Box<dyn FnMut(JsValue)>);
    sock.lock()
        .set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();

    Ok(sock)
}
