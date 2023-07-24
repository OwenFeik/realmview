// The #[wasm_bindgen(start)] call is needed but Clippy doesn't see that.
#![allow(clippy::unused_unit)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Mutex;

use wasm_bindgen::prelude::*;

use crate::bridge::{
    expose_closure_f64, expose_closure_f64x2_string, expose_closure_string_in,
    expose_closure_string_out, flog, log, request_animation_frame,
};
use crate::client::Client;
use crate::dom::menu::Menu;
use crate::viewport::Viewport;

pub type VpRef = Rc<Mutex<Viewport>>;

fn lock_and<T: FnOnce(&mut Viewport)>(vp: &VpRef, action: T) {
    if let Ok(mut lock) = vp.try_lock() {
        action(&mut lock);
    } else {
        log("Failed to lock for handler.");
    }
}

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
        Err(e) => return logged_error(&format!("Failed to connect to game: {e}")),
    };

    let vp = match Viewport::new(client) {
        Ok(s) => Rc::new(Mutex::new(s)),
        Err(_) => return logged_error("Failed to create viewport."),
    };

    lock_and(&vp, |lock| {
        lock.add_menu(Menu::new(vp.clone(), scene::perms::Role::Owner))
    });

    // This closure acquires the lock on the Viewport, then exports the scene
    // as a binary blob. This allows the front end to pull out the binary
    // representation of the scene to send back to the server.
    let vp_ref = vp.clone();
    let export_closure = Closure::wrap(Box::new(move || {
        if let Ok(lock) = vp_ref.try_lock() {
            base64::encode(lock.scene.export())
        } else {
            log("Failed to lock for export.");
            String::new()
        }
    }) as Box<dyn FnMut() -> String>);
    expose_closure_string_out("export_scene", &export_closure);
    export_closure.forget();

    let vp_ref = vp.clone();
    let load_scene_closure = Closure::wrap(Box::new(move |vp_b64: String| {
        if let Ok(bytes) = base64::decode(vp_b64) {
            if let Ok(scene) = bincode::deserialize(&bytes) {
                lock_and(&vp_ref, |vp| vp.replace_scene(scene));
            }
        }
    }) as Box<dyn FnMut(String)>);
    expose_closure_string_in("load_scene", &load_scene_closure);
    load_scene_closure.forget();

    let vp_ref = vp.clone();
    let new_scene_closure = Closure::wrap(Box::new(move |id: f64| {
        lock_and(&vp_ref, |vp| vp.scene.new_scene(id as i64));
    }) as Box<dyn FnMut(f64)>);
    expose_closure_f64("new_scene", &new_scene_closure);
    new_scene_closure.forget();

    let vp_ref = vp.clone();
    let new_sprite_closure = Closure::wrap(Box::new(move |w: f64, h: f64, media_key: String| {
        let texture = crate::render::parse_media_key(&media_key);
        lock_and(&vp_ref, |vp| {
            let at = vp.placement_tile();
            vp.scene.new_sprite_at(
                Some(scene::SpriteVisual::Texture {
                    id: texture,
                    shape: scene::Shape::Rectangle,
                }),
                None,
                scene::Rect::at(at, w as f32, h as f32),
            );
        })
    }) as Box<dyn FnMut(f64, f64, String)>);
    expose_closure_f64x2_string("new_sprite", &new_sprite_closure);
    new_sprite_closure.forget();

    let vp_ref = vp.clone();
    let set_scene_list_closure = Closure::wrap(Box::new(move |json: String| {
        if let Some(scenes) = parse_json::<Vec<(String, String)>>(&json) {
            lock_and(&vp_ref, move |vp| vp.set_scene_list(scenes));
        }
    }) as Box<dyn FnMut(String)>);
    expose_closure_string_in("set_scene_list", &set_scene_list_closure);
    set_scene_list_closure.forget();

    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        if let Ok(mut lock) = vp.lock() {
            lock.animation_frame();
        } else {
            log("Failed to lock viewport for animation frame.");
        }
        request_animation_frame(f.borrow().as_ref().unwrap()).unwrap();
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap()).unwrap();

    Ok(())
}
