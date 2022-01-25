use std::rc::Rc;
use std::cell::RefCell;

use wasm_bindgen::prelude::*;

use crate::bridge::request_animation_frame;
use crate::viewport::Viewport;

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let mut scene = Viewport::new().unwrap();

    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        scene.animation_frame();
        request_animation_frame(f.borrow().as_ref().unwrap()).unwrap();
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap()).unwrap();

    Ok(())
}
