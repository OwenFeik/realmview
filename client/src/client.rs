use std::rc::Rc;

use bincode::{deserialize, serialize};
use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen::{prelude::*, JsCast};
use parking_lot::Mutex;
use web_sys::{ErrorEvent, MessageEvent, WebSocket};

use scene::comms::{ClientEvent, ServerEvent};

use crate::bridge::{log, websocket_url, JsError};

pub struct Client {
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

        let incoming_events = Rc::new(Mutex::new(Vec::new()));

        // More performant than Blob for small payloads, per the wasm-bindgen
        // example at
        // https://rustwasm.github.io/wasm-bindgen/examples/websockets.html
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        let event_queue = incoming_events.clone();
        let onmessage = Closure::wrap(Box::new(move |e: MessageEvent| {
            match deserialise_message(e.data()) {
                Ok(e) => event_queue.lock().push(e),
                Err(JsError::ResourceError(s)) => log(s),
                Err(JsError::TypeError(s)) => log(s)
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        onmessage.forget();

        let onerror = Closure::wrap(Box::new(move |e: ErrorEvent| {
            log(&format!("WebSocket error: {:?}", e));
        }) as Box<dyn FnMut(ErrorEvent)>);
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();

        Ok(Some(Client {
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

    fn _send_event(&self, message: &[u8], retry: bool) {
        if let Err(_) = self.sock.send_with_u8_array(message) {
            if retry {
                self._send_event(message, false);
            }
            else {
                log("Failed to send event.");
            }
        }
    }

    pub fn send_event(&self, event: &ClientEvent) {
        if let Ok(m) = serialize(event) {
            self._send_event(&m, true);
        }
    }
}

fn deserialise_message(message: JsValue) -> Result<ServerEvent, JsError> {
    match message.dyn_into::<ArrayBuffer>() {
        Ok(b) => match deserialize(&Uint8Array::new(&b).to_vec()) {
            Ok(e) => Ok(e),
            Err(_) => Err(JsError::ResourceError("WebSocket message deserialisation failed."))
        },
        Err(_) => Err(JsError::TypeError("WebSocket message could not be cast to ArrayBuffer."))
    }
}
