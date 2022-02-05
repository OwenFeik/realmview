use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use bincode::{deserialize, serialize};
use js_sys::{Array, ArrayBuffer, Uint8Array};
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{MessageEvent, WebSocket, ErrorEvent};

use scene::comms::{ServerEvent, ClientEvent};

use crate::bridge::{log, JsError};

type ServerEvents = Rc<RefCell<VecDeque<ServerEvent>>>;

pub struct Client {
    sock: WebSocket,
    incoming_events: Array
}

impl Client {
    pub fn new(url: &str) -> Result<Client, JsError> {
        let ws = match WebSocket::new(url) {
            Ok(ws) => ws,
            Err(_) => return Err(JsError::ResourceError("Failed to open WebSocket."))
        };

        let incoming_events = Array::new();

        // More performant than Blob for small payloads, per the wasm-bindgen
        // example https://rustwasm.github.io/wasm-bindgen/examples/websockets.html
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);
    
        let event_queue = incoming_events.clone();
        let onmessage = Closure::wrap(Box::new(move |e: MessageEvent| {
            event_queue.push(&e.data());            
        }) as Box<dyn FnMut(MessageEvent)>);
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        onmessage.forget();
    
        let onerror = Closure::wrap(Box::new(move |e: ErrorEvent| {
            log(&format!("WebSocket error: {:?}", e));
        }) as Box<dyn FnMut(ErrorEvent)>);
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();
    

        Ok(Client { sock: ws, incoming_events })
    }

    // Returns vector of events ordered from newest to oldest.
    pub fn recv_events(&self) -> Vec<ServerEvent> {
        let mut events = Vec::new();

        loop {
            let message = self.incoming_events.pop();

            if message.is_undefined() {
                break;
            }
            
            if let Ok(buf) = message.dyn_into::<ArrayBuffer>() {
                if let Ok(event) = deserialize(&Uint8Array::new(&buf).to_vec()) {
                    events.push(event);
                }
                else {
                    log("Error deserialising WebSocket message.");
                }
            }
            else {
                log("Error parsing WebSocket message as ArrayBuffer.");
            }

        }

        events
    }

    fn _send_event(&self, message: &[u8], retry: bool) {
        if let Err(_) = self.sock.send_with_u8_array(message) {
            if retry {
                self._send_event(message, false);
            }
        } 
    }

    pub fn send_event(&self, event: &ClientEvent) {
        if let Ok(m) = serialize(event) {
            self._send_event(&m, true);
        }
    } 
}
