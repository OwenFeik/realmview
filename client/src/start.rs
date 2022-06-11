use std::cell::RefCell;
use std::rc::Rc;

use js_sys::Array;
use parking_lot::Mutex;
use wasm_bindgen::prelude::*;

use crate::bridge::{
    expose_closure, expose_closure_array, expose_closure_f64, expose_closure_f64_bool,
    expose_closure_f64_f64, expose_closure_f64_string, expose_closure_string_in,
    expose_closure_string_out, layer_info, log, request_animation_frame,
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
    expose_closure_string_out("export_scene", &export_closure);
    export_closure.forget();

    let scene_ref = scene.clone();
    let load_scene_closure = Closure::wrap(Box::new(move |scene_b64: String| {
        let s = match base64::decode(&scene_b64) {
            Ok(b) => match bincode::deserialize(&b) {
                Ok(s) => s,
                _ => return,
            },
            _ => return,
        };
        scene_ref.lock().replace_scene(s);
    }) as Box<dyn FnMut(String)>);
    expose_closure_string_in("load_scene", &load_scene_closure);
    load_scene_closure.forget();

    let scene_ref = scene.clone();
    let new_scene_closure = Closure::wrap(Box::new(move |id: f64| {
        scene_ref.lock().new_scene(id as i64);
    }) as Box<dyn FnMut(f64)>);
    expose_closure_f64("new_scene", &new_scene_closure);
    new_scene_closure.forget();

    let scene_ref = scene.clone();
    let new_sprite_closure = Closure::wrap(Box::new(move |id: f64, layer: f64| {
        scene_ref.lock().new_sprite(id as i64, layer as i64);
    }) as Box<dyn FnMut(f64, f64)>);
    expose_closure_f64_f64("new_sprite", &new_sprite_closure);
    new_sprite_closure.forget();

    let scene_ref = scene.clone();
    let rename_layer_closure = Closure::wrap(Box::new(move |id: f64, title: String| {
        scene_ref.lock().rename_layer(id as i64, title);
    }) as Box<dyn FnMut(f64, String)>);
    expose_closure_f64_string("rename_layer", &rename_layer_closure);
    rename_layer_closure.forget();

    let scene_ref = scene.clone();
    let layer_visibility_closure = Closure::wrap(Box::new(move |id: f64, visible: bool| {
        scene_ref.lock().set_layer_visible(id as i64, visible);
    }) as Box<dyn FnMut(f64, bool)>);
    expose_closure_f64_bool("layer_visible", &layer_visibility_closure);
    layer_visibility_closure.forget();

    let scene_ref = scene.clone();
    let layer_locked_closure = Closure::wrap(Box::new(move |id: f64, locked: bool| {
        scene_ref.lock().set_layer_locked(id as i64, locked);
    }) as Box<dyn FnMut(f64, bool)>);
    expose_closure_f64_bool("layer_locked", &layer_locked_closure);
    layer_locked_closure.forget();

    let scene_ref = scene.clone();
    let scene_layers_closure = Closure::wrap(
        Box::new(move || layer_info(scene_ref.lock().layers())) as Box<dyn FnMut() -> Array>,
    );
    expose_closure_array("scene_layers", &scene_layers_closure);
    scene_layers_closure.forget();

    let scene_ref = scene.clone();
    let new_layer_closure = Closure::wrap(Box::new(move || {
        scene_ref.lock().new_layer();
    }) as Box<dyn FnMut()>);
    expose_closure("new_layer", &new_layer_closure);
    new_layer_closure.forget();

    let scene_ref = scene.clone();
    let move_layer_closure = Closure::wrap(Box::new(move |id: f64, up: bool| {
        scene_ref.lock().move_layer(id as i64, up);
    }) as Box<dyn FnMut(f64, bool)>);
    expose_closure_f64_bool("move_layer", &move_layer_closure);
    move_layer_closure.forget();

    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        scene.lock().animation_frame();
        request_animation_frame(f.borrow().as_ref().unwrap()).unwrap();
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap()).unwrap();

    Ok(())
}
