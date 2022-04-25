use std::cell::RefCell;
use std::rc::Rc;

use js_sys::Uint8Array;
use parking_lot::Mutex;
use wasm_bindgen::prelude::*;

use crate::bridge::{
    expose_closure_string_string, expose_closure_u8_array, log, request_animation_frame,
};
use crate::client::Client;
use crate::viewport::Viewport;

fn logged_error<T>(error_message: &str) -> Result<T, JsValue> {
    log(error_message);
    Err(wasm_bindgen::JsValue::from_str(error_message))
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let client = match Client::new() {
        Ok(c) => c,
        Err(_) => return logged_error("Failed to connect to game."),
    };

    let scene = match Viewport::new(client) {
        Ok(s) => Rc::new(Mutex::new(s)),
        Err(_) => return logged_error("Failed to create viewport."),
    };

    // This closure acquires the lock on the Viewport, then exports the scene
    // as a binary blob. This allows the front end to pull out the binary
    // representation of the scene to send back to the server.
    let scene_ref = scene.clone();
    let export_closure = Closure::wrap(Box::new(move || {
        let data = scene_ref.lock().export();
        let ary = Uint8Array::new_with_length(data.len() as u32);
        ary.copy_from(&data);
        ary
    }) as Box<dyn FnMut() -> Uint8Array>);
    expose_closure_u8_array("export_scene", &export_closure);
    export_closure.forget();

    let scene_ref = scene.clone();
    let set_id_closure = Closure::wrap(Box::new(move |scene_b64: String| {
        log(&scene_b64);
        let s = match base64::decode(&scene_b64) {
            Ok(b) => match bincode::deserialize(&b) {
                Ok(s) => s,
                Err(e) => return format!("Deserialisation error: {}", e),
            },
            Err(e) => return format!("Decoding error: {}", e),
        };
        scene_ref.lock().replace_scene(s);
        "Saved successfully.".to_string()
    }) as Box<dyn FnMut(String) -> String>);
    expose_closure_string_string("load_scene", &set_id_closure);
    set_id_closure.forget();

    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        scene.lock().animation_frame();
        request_animation_frame(f.borrow().as_ref().unwrap()).unwrap();
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap()).unwrap();

    Ok(())
}
