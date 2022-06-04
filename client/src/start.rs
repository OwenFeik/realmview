use std::cell::RefCell;
use std::rc::Rc;

use parking_lot::Mutex;
use wasm_bindgen::prelude::*;

use crate::bridge::{
    expose_closure_string, expose_closure_string_string, log, request_animation_frame, expose_closure_f64,
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
        base64::encode(data)
    }) as Box<dyn FnMut() -> String>);
    expose_closure_string("export_scene", &export_closure);
    export_closure.forget();

    let scene_ref = scene.clone();
    let load_scene_closure = Closure::wrap(Box::new(move |scene_b64: String| {
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
    expose_closure_string_string("load_scene", &load_scene_closure);
    load_scene_closure.forget();

    let scene_ref = scene.clone();
    let new_scene_closure = Closure::wrap(Box::new(move |id: f64| {
        scene_ref.lock().new_scene(id as i64);
    }) as Box<dyn FnMut(f64)>);
    expose_closure_f64("new_scene", &new_scene_closure);
    new_scene_closure.forget();

    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        scene.lock().animation_frame();
        request_animation_frame(f.borrow().as_ref().unwrap()).unwrap();
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap()).unwrap();

    Ok(())
}
