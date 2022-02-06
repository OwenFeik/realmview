use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;

use crate::bridge::request_animation_frame;
use crate::client::Client;
use crate::viewport::Viewport;

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let client = match Client::new() {
        Ok(c) => c,
        Err(_) => return Err(wasm_bindgen::JsValue::from_str("Failed to connect to game.")),
    };

    let mut scene = Viewport::new(client).unwrap();

    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        scene.animation_frame();
        request_animation_frame(f.borrow().as_ref().unwrap()).unwrap();
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap()).unwrap();

    Ok(())
}
