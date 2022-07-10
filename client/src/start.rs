use std::cell::RefCell;
use std::rc::Rc;

use parking_lot::Mutex;
use wasm_bindgen::prelude::*;

use crate::bridge::{
    expose_closure, expose_closure_f64, expose_closure_f64_bool, expose_closure_f64_f64,
    expose_closure_f64_string, expose_closure_string_in, expose_closure_string_out, log,
    request_animation_frame,
};
use crate::client::Client;
use crate::viewport::Viewport;

fn logged_error<T>(error_message: &str) -> Result<T, JsValue> {
    log(error_message);
    Err(wasm_bindgen::JsValue::from_str(error_message))
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    let client = match Client::new() {
        Ok(c) => c,
        Err(_) => return logged_error("Failed to connect to game."),
    };

    let vp = match Viewport::new(client) {
        Ok(s) => Rc::new(Mutex::new(s)),
        Err(_) => return logged_error("Failed to create viewport."),
    };

    // This closure acquires the lock on the Viewport, then exports the scene
    // as a binary blob. This allows the front end to pull out the binary
    // representation of the scene to send back to the server.
    let vp_ref = vp.clone();
    let export_closure = Closure::wrap(Box::new(move || {
        let data = vp_ref.lock().scene.export();
        base64::encode(data)
    }) as Box<dyn FnMut() -> String>);
    expose_closure_string_out("export_scene", &export_closure);
    export_closure.forget();

    let vp_ref = vp.clone();
    let load_scene_closure = Closure::wrap(Box::new(move |vp_b64: String| {
        let s = match base64::decode(&vp_b64) {
            Ok(b) => match bincode::deserialize(&b) {
                Ok(s) => s,
                _ => return,
            },
            _ => return,
        };
        vp_ref.lock().scene.replace_scene(s);
    }) as Box<dyn FnMut(String)>);
    expose_closure_string_in("load_scene", &load_scene_closure);
    load_scene_closure.forget();

    let vp_ref = vp.clone();
    let new_scene_closure = Closure::wrap(Box::new(move |id: f64| {
        vp_ref.lock().scene.new_scene(id as i64);
    }) as Box<dyn FnMut(f64)>);
    expose_closure_f64("new_scene", &new_scene_closure);
    new_scene_closure.forget();

    let vp_ref = vp.clone();
    let new_sprite_closure = Closure::wrap(Box::new(move |layer: f64, media_key: String| {
        let texture = crate::programs::parse_media_key(&media_key);
        vp_ref.lock().scene.new_sprite(texture, layer as i64);
    }) as Box<dyn FnMut(f64, String)>);
    expose_closure_f64_string("new_sprite", &new_sprite_closure);
    new_sprite_closure.forget();

    let vp_ref = vp.clone();
    let clone_sprite_closure = Closure::wrap(Box::new(move |id: f64| {
        vp_ref.lock().scene.clone_sprite(id as i64);
    }) as Box<dyn FnMut(f64)>);
    expose_closure_f64("clone_sprite", &clone_sprite_closure);
    clone_sprite_closure.forget();

    let vp_ref = vp.clone();
    let remove_sprite_closure = Closure::wrap(Box::new(move |id: f64| {
        vp_ref.lock().scene.remove_sprite(id as i64);
    }) as Box<dyn FnMut(f64)>);
    expose_closure_f64("remove_sprite", &remove_sprite_closure);
    remove_sprite_closure.forget();

    let vp_ref = vp.clone();
    let sprite_layer_closure = Closure::wrap(Box::new(move |id: f64, layer: f64| {
        vp_ref.lock().scene.sprite_layer(id as i64, layer as i64);
    }) as Box<dyn FnMut(f64, f64)>);
    expose_closure_f64_f64("sprite_layer", &sprite_layer_closure);
    sprite_layer_closure.forget();

    let vp_ref = vp.clone();
    let sprite_details_closure = Closure::wrap(Box::new(move |id: f64, json: String| {
        let id = id as i64;
        if let Ok(details) = serde_json::from_str::<crate::interactor::SpriteDetails>(&json) {
            let mut lock = vp_ref.lock();

            if let Some(x) = details.x {
                lock.scene.sprite_dimension(id, scene::Dimension::X, x);
            }

            if let Some(y) = details.y {
                lock.scene.sprite_dimension(id, scene::Dimension::Y, y);
            }

            if let Some(w) = details.w {
                lock.scene.sprite_dimension(id, scene::Dimension::W, w);
            }

            if let Some(h) = details.h {
                lock.scene.sprite_dimension(id, scene::Dimension::H, h);
            }
        }
    }) as Box<dyn FnMut(f64, String)>);
    expose_closure_f64_string("sprite_details", &sprite_details_closure);
    sprite_details_closure.forget();

    let vp_ref = vp.clone();
    let rename_layer_closure = Closure::wrap(Box::new(move |id: f64, title: String| {
        vp_ref.lock().scene.rename_layer(id as i64, title);
    }) as Box<dyn FnMut(f64, String)>);
    expose_closure_f64_string("rename_layer", &rename_layer_closure);
    rename_layer_closure.forget();

    let vp_ref = vp.clone();
    let layer_visibility_closure = Closure::wrap(Box::new(move |id: f64, visible: bool| {
        vp_ref.lock().scene.set_layer_visible(id as i64, visible);
    }) as Box<dyn FnMut(f64, bool)>);
    expose_closure_f64_bool("layer_visible", &layer_visibility_closure);
    layer_visibility_closure.forget();

    let vp_ref = vp.clone();
    let layer_locked_closure = Closure::wrap(Box::new(move |id: f64, locked: bool| {
        vp_ref.lock().scene.set_layer_locked(id as i64, locked);
    }) as Box<dyn FnMut(f64, bool)>);
    expose_closure_f64_bool("layer_locked", &layer_locked_closure);
    layer_locked_closure.forget();

    let vp_ref = vp.clone();
    let new_layer_closure = Closure::wrap(Box::new(move || {
        vp_ref.lock().scene.new_layer();
    }) as Box<dyn FnMut()>);
    expose_closure("new_layer", &new_layer_closure);
    new_layer_closure.forget();

    let vp_ref = vp.clone();
    let remove_layer_closure = Closure::wrap(Box::new(move |id: f64| {
        vp_ref.lock().scene.remove_layer(id as i64);
    }) as Box<dyn FnMut(f64)>);
    expose_closure_f64("remove_layer", &remove_layer_closure);
    remove_layer_closure.forget();

    let vp_ref = vp.clone();
    let move_layer_closure = Closure::wrap(Box::new(move |id: f64, up: bool| {
        vp_ref.lock().scene.move_layer(id as i64, up);
    }) as Box<dyn FnMut(f64, bool)>);
    expose_closure_f64_bool("move_layer", &move_layer_closure);
    move_layer_closure.forget();

    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        vp.lock().animation_frame();
        request_animation_frame(f.borrow().as_ref().unwrap()).unwrap();
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap()).unwrap();

    Ok(())
}
