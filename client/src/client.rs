use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicI32};
use std::sync::Mutex;

use anyhow::anyhow;
use bincode::{deserialize, serialize};
use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};

use crate::bridge::{flog, game_over_redirect, log, log_js_value, websocket_url};
use crate::scene::comms::{ClientEvent, ClientMessage, ServerEvent};

pub struct Client {
    sock: Sock,

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
        let sock = connect_websocket(url, incoming_events)?;

        Ok(Some(Client { sock, counter: 0 }))
    }

    // Returns vector of events ordered from newest to oldest.
    // This order is chosen because it allows popping from the end in order to
    // apply events in the correct order.
    pub fn events(&mut self) -> Vec<ServerEvent> {
        self.counter += 1;
        if self.counter >= Self::PING_INTERVAL_FRAMES {
            self.ping();
        }

        self.sock.load_events()
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

struct Sock {
    socket: SockRef,
    events: EventsRef,
    connecting: Rc<AtomicBool>,
}

impl Sock {
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
        if let Ok(data) = serialize(message) {
            if let Ok(sock) = self.socket.try_lock() {
                if let Err(v) = sock.send_with_u8_array(&data) {
                    log("Failed to send event. Reason:");
                    log_js_value(&v);
                    if retry {
                        log("Retrying send event.");
                        self.send_message(message, false);
                    }
                }
            } else {
                log("Failed to lock client socket to send.");
            }
        } else {
            log("Failed to serialise message to send.");
        }
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

    Ok(ws)
}

const MAX_RETRIES: u32 = 100;
const RETRY_INTERVAL_MS: i32 = 1000;

/// Attempt to reconnect a websocket by opening a new one and replacing the
/// value in the mutex. Will retry up to MAX_RETRIES times, every
/// RETRY_INTERVAL_MS milliseconds.
fn reconnect_websocket(url: String, events: EventsRef, socket: SockRef) {
    const NO_HANDLE: i32 = -1;
    let Some(window) = web_sys::window() else { return; };
    let handle = Rc::new(AtomicI32::new(NO_HANDLE));

    let handle_ref = handle.clone();
    let clear_timeout = move || {
        let handle = handle_ref.load(std::sync::atomic::Ordering::Acquire);
        if handle != NO_HANDLE && let Some(window) = web_sys::window() {
            window.clear_interval_with_handle(handle);
        }
    };

    let mut num_retries = 0;
    let callback = Closure::wrap(Box::new(move || {
        if num_retries >= MAX_RETRIES {
            flog!("Failed to reconnect after {num_retries} retries. Redirecting.");
            game_over_redirect();
            clear_timeout();
        }

        if let Ok(replacement) = create_websocket(&url, events.clone()) {
            if let Ok(mut old) = socket.try_lock() {
                *old = replacement;
                add_handlers(url.clone(), events.clone(), socket.clone());
                log("Successfully reconnected WebSocket.");
            } else {
                log("Failed to lock socket to set replacement.");
                replacement.close().ok();
            }
            clear_timeout();
        }

        num_retries += 1;
    }) as Box<dyn FnMut()>);

    match window.set_interval_with_callback_and_timeout_and_arguments_0(
        callback.as_ref().unchecked_ref(),
        RETRY_INTERVAL_MS,
    ) {
        Ok(h) => handle.store(h, std::sync::atomic::Ordering::Release),
        Err(error) => {
            flog!("Failed to set reconnect interval. Error:");
            log_js_value(&error);
        }
    }
    callback.forget();
}

/// Add handlers to a websocket wrapped in a mutex to reconnect it in the case
/// that it closes inadvertently.
fn add_handlers(url: String, events: EventsRef, sock: SockRef) {
    let url_ref = url.clone();
    let sock_ref = sock.clone();
    let events_ref = events.clone();
    let onerror = Closure::wrap(Box::new(move |e: ErrorEvent| {
        flog!("Attempting reconnect. Closed due to error:");
        log_js_value(&e);
        reconnect_websocket(url_ref.clone(), events_ref.clone(), sock_ref.clone());
    }) as Box<dyn FnMut(ErrorEvent)>);

    if let Ok(lock) = sock.try_lock() {
        lock.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();
    } else {
        log("Failed to set onerror handler for socket.");
    }

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

    if let Ok(lock) = sock.try_lock() {
        lock.set_onclose(Some(onclose.as_ref().unchecked_ref()));
        onclose.forget();
    } else {
        log("Failed to set onclose handler for socket.");
    }
}

/// Connects a websocket, returning a Mutex to that websocket. In the event
/// that the socket is closed, this will replace the value in the Mutex with a
/// new socket, setting ready false in the interim.
fn connect_websocket(url: String, events: EventsRef) -> anyhow::Result<Sock> {
    let socket = Rc::new(Mutex::new(create_websocket(&url, events.clone())?));
    add_handlers(url, events.clone(), socket.clone());
    Ok(Sock {
        socket,
        events,
        connecting: Rc::new(AtomicBool::new(false)),
    })
}
