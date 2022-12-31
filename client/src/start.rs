// The #[wasm_bindgen(start)] call is needed but Clippy doesn't see that.
#![allow(clippy::unused_unit)]

use std::cell::RefCell;
use std::rc::Rc;

use parking_lot::Mutex;
use wasm_bindgen::prelude::*;

use crate::bridge::{
    expose_closure, expose_closure_f64, expose_closure_f64_bool, expose_closure_f64_f64,
    expose_closure_f64_string, expose_closure_f64x3_string, expose_closure_string_in,
    expose_closure_string_out, flog, log, request_animation_frame,
};
use crate::client::Client;
use crate::viewport::{Tool, Viewport};

fn logged_error<T>(error_message: &str) -> Result<T, JsValue> {
    log(error_message);
    Err(wasm_bindgen::JsValue::from_str(error_message))
}

fn parse_json<'a, T: serde::Deserialize<'a>>(json: &'a str) -> Option<T> {
    if let Ok(val) = serde_json::from_str::<T>(json) {
        Some(val)
    } else {
        flog!("Failed to parse JSON: {json}");
        None
    }
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

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
        let s = match base64::decode(vp_b64) {
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
    let change_scene_closure = Closure::wrap(Box::new(move |scene_key: String| {
        vp_ref.lock().scene.change_scene(scene_key);
    }) as Box<dyn FnMut(String)>);
    expose_closure_string_in("change_scene", &change_scene_closure);
    change_scene_closure.forget();

    let vp_ref = vp.clone();
    let scene_details_closure = Closure::wrap(Box::new(move |json: String| {
        if let Ok(details) = serde_json::from_str::<crate::interactor::details::SceneDetails>(&json)
        {
            let mut lock = vp_ref.lock();
            lock.scene.scene_details(details);
            crate::bridge::set_scene_details(lock.scene.get_scene_details());
            crate::bridge::flog!("{:?}", lock.scene.get_scene_details());
        }
    }) as Box<dyn FnMut(String)>);
    expose_closure_string_in("scene_details", &scene_details_closure);
    scene_details_closure.forget();

    let vp_ref = vp.clone();
    let new_sprite_closure = Closure::wrap(Box::new(
        move |layer: f64, w: f64, h: f64, media_key: String| {
            let texture = crate::render::parse_media_key(&media_key);
            let mut lock = vp_ref.lock();
            let at = lock.placement_tile();
            lock.scene.new_sprite_at(
                Some(scene::SpriteVisual::Texture {
                    id: texture,
                    shape: scene::SpriteShape::Rectangle,
                }),
                Some(layer as i64),
                scene::Rect::at(at, w as f32, h as f32),
            );
        },
    ) as Box<dyn FnMut(f64, f64, f64, String)>);
    expose_closure_f64x3_string("new_sprite", &new_sprite_closure);
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
        if let Some(details) = parse_json(&json) {
            vp_ref.lock().scene.sprite_details(id, details);
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

    let vp_ref = vp.clone();
    let select_layer_closure = Closure::wrap(Box::new(move |id: f64| {
        vp_ref.lock().scene.select_layer(id as i64);
    }) as Box<dyn FnMut(f64)>);
    expose_closure_f64("select_layer", &select_layer_closure);
    select_layer_closure.forget();

    let vp_ref = vp.clone();
    let select_tool_closure = Closure::wrap(Box::new(move |tool: String| {
        vp_ref
            .lock()
            .set_tool(parse_json(&format!("\"{tool}\"")).unwrap_or(Tool::Select));
    }) as Box<dyn FnMut(String)>);
    expose_closure_string_in("select_tool", &select_tool_closure);
    select_tool_closure.forget();

    let vp_ref = vp.clone();
    let draw_details_closure = Closure::wrap(Box::new(move |json: String| {
        if let Some(details) = parse_json(&json) {
            vp_ref.lock().scene.update_draw_details(details);
        }
    }) as Box<dyn FnMut(String)>);
    expose_closure_string_in("draw_details", &draw_details_closure);
    draw_details_closure.forget();

    let vp_ref = vp.clone();
    let set_fog_brush_closure = Closure::wrap(Box::new(move |size: f64| {
        vp_ref.lock().scene.set_fog_brush(size as u32);
    }) as Box<dyn FnMut(f64)>);
    expose_closure_f64("set_fog_brush", &set_fog_brush_closure);
    set_fog_brush_closure.forget();

    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        vp.lock().animation_frame();
        request_animation_frame(f.borrow().as_ref().unwrap()).unwrap();
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap()).unwrap();

    Ok(())
}
