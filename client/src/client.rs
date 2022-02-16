use std::sync::atomic::AtomicBool;
use std::{rc::Rc, sync::atomic::Ordering};

use bincode::{deserialize, serialize};
use js_sys::{ArrayBuffer, Uint8Array};
use parking_lot::Mutex;
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{ErrorEvent, MessageEvent, WebSocket};

use scene::comms::{ClientEvent, ClientMessage, ServerEvent};

use crate::bridge::{log, log_js_value, websocket_url, JsError};

pub struct Client {
    ready: Rc<AtomicBool>,
    sock: WebSocket,
    incoming_events: Rc<Mutex<Vec<ServerEvent>>>,
}

impl Client {
    pub fn new() -> Result<Option<Client>, JsError> {
        let ws = match websocket_url() {
            Ok(Some(url)) => match WebSocket::new(&url) {
                Ok(ws) => ws,
                Err(_) => return Err(JsError::ResourceError("Failed to open WebSocket.")),
            },
            _ => return Ok(None),
        };

        let ready = Rc::new(AtomicBool::new(false));
        let incoming_events = Rc::new(Mutex::new(Vec::new()));

        // More performant than Blob for small payloads, per the wasm-bindgen
        // example at
        // https://rustwasm.github.io/wasm-bindgen/examples/websockets.html
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        let event_queue = incoming_events.clone();
        let onmessage =
            Closure::wrap(
                Box::new(move |e: MessageEvent| match deserialise_message(e.data()) {
                    Ok(e) => event_queue.lock().push(e),
                    Err(JsError::ResourceError(s)) => log(s),
                    Err(JsError::TypeError(s)) => log(s),
                }) as Box<dyn FnMut(MessageEvent)>,
            );
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        onmessage.forget();

        let onerror = Closure::wrap(Box::new(move |e: ErrorEvent| {
            log(&format!("WebSocket error: {:?}", e));
        }) as Box<dyn FnMut(ErrorEvent)>);
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();

        let ready_clone = ready.clone();
        let onopen = Closure::wrap(
            Box::new(move |_| ready_clone.store(true, Ordering::Relaxed))
                as Box<dyn FnMut(JsValue)>,
        );
        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        onopen.forget();

        Ok(Some(Client {
            ready,
            sock: ws,
            incoming_events,
        }))
    }

    // Returns vector of events ordered from newest to oldest.
    // This order is chosen because it allows popping from the end in order to
    // apply events in the correct order.
    pub fn events(&self) -> Vec<ServerEvent> {
        let mut events = self.incoming_events.lock();
        let mut ret = Vec::new();
        ret.append(&mut events);
        ret.reverse();
        ret
    }

    fn _send_message(&self, message: &[u8], retry: bool) {
        if let Err(v) = self.sock.send_with_u8_array(message) {
            if retry {
                self._send_message(message, false);
            } else {
                log("Failed to send event. Reason:");
                log_js_value(&v);
            }
        }
    }

    pub fn send_message(&self, message: &ClientMessage) {
        if let Ok(m) = serialize(message) {
            self._send_message(&m, true);
        }
    }

    pub fn ping(&self) {
        self.send_message(&ClientMessage {
            id: 0,
            event: ClientEvent::Ping,
        });
    }
}

fn deserialise_message(message: JsValue) -> Result<ServerEvent, JsError> {
    match message.dyn_into::<ArrayBuffer>() {
        Ok(b) => match deserialize(&Uint8Array::new(&b).to_vec()) {
            Ok(e) => Ok(e),
            Err(_) => Err(JsError::ResourceError(
                "WebSocket message deserialisation failed.",
            )),
        },
        Err(_) => Err(JsError::TypeError(
            "WebSocket message could not be cast to ArrayBuffer.",
        )),
    }
}
