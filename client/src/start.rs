// The #[wasm_bindgen(start)] call is needed but Clippy doesn't see that.
#![allow(clippy::unused_unit)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Mutex;

use wasm_bindgen::{prelude::*, JsCast};

use crate::bridge::{
    console_log, expose_closure_f64x2_string, expose_closure_string_out, load_project, log,
    request_animation_frame,
};
use crate::dom::menu::Menu;
use crate::viewport::{self, Viewport};

pub type VpRef = Rc<Mutex<Viewport>>;

fn logged_error<T>(error_message: &str) -> Result<T, JsValue> {
    console_log(error_message);
    Err(wasm_bindgen::JsValue::from_str(error_message))
}

fn parse_json<'a, T: serde::Deserialize<'a>>(json: &'a str) -> Option<T> {
    if let Ok(val) = serde_json::from_str::<T>(json) {
        Some(val)
    } else {
        log!("Failed to parse JSON: {json}");
        None
    }
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    viewport::lock_and(|lock| lock.add_menu(Menu::new(scene::perms::Role::Owner)));

    let new_sprite_closure = Closure::wrap(Box::new(move |w: f64, h: f64, media_key: String| {
        let texture = crate::render::parse_media_key(&media_key);
        viewport::lock_and(|vp| {
            let at = vp.placement_tile();
            vp.int.new_sprite_at(
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

    let active_scene_closure =
        Closure::wrap(
            Box::new(move || viewport::lock_and(|vp| vp.int.scene_uuid()))
                as Box<dyn FnMut() -> String>,
        );
    expose_closure_string_out("active_scene", &active_scene_closure);
    active_scene_closure.forget();

    let before_unload_closure: Closure<dyn FnMut() -> Option<String>> = Closure::new(move || {
        viewport::lock_and(|vp| {
            if vp.int.save_required() {
                vp.save();
                Some(String::new())
            } else {
                None
            }
        })
    });
    web_sys::window()
        .unwrap()
        .set_onbeforeunload(Some(before_unload_closure.as_ref().unchecked_ref()));
    before_unload_closure.forget();

    load_project().ok();

    let f = Rc::new(RefCell::new(None));
    let g = f.clone();
    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        viewport::lock_and(|vp| vp.animation_frame());
        request_animation_frame(f.borrow().as_ref().unwrap()).unwrap();
    }) as Box<dyn FnMut()>));
    request_animation_frame(g.borrow().as_ref().unwrap()).unwrap();

    Ok(())
}
