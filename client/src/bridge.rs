use std::rc::Rc;

use js_sys::Array;
use serde_derive::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    Blob, Document, FileReader, HtmlCanvasElement, HtmlElement, HtmlImageElement, HtmlInputElement,
    ProgressEvent, UiEvent, Url, WebGl2RenderingContext, Window,
};

use crate::interactor::SceneDetails;
use crate::interactor::SpriteDetails;
use crate::render::Renderer;
use crate::scene::{Id, Layer, Rect, Sprite};
use crate::viewport::ViewportPoint;

#[wasm_bindgen]
extern "C" {
    // Returns an array where loaded texture images will be placed once ready.
    fn get_texture_queue() -> Array;

    // Causes the texture with this ID to be loaded as an image and added to
    // the texture queue once ready.
    pub fn load_texture(media_key: String);

    // Shows a dropdown with actions for the specified sprite at (x, y) on the
    // scene canvas.
    pub fn sprite_dropdown(sprite: Id, x: f32, y: f32);

    // Shows or hides the relevant UI elements given a role integer.
    pub fn update_interface(role: i32);

    // Updates the sprite menu to refer to this sprite.
    #[wasm_bindgen(js_name = set_selected_sprite)]
    fn _set_selected_sprite(sprite_json: String);

    // Clears data from the sprite menu.
    #[wasm_bindgen]
    pub fn clear_selected_sprite();

    #[wasm_bindgen(js_name = set_scene_details)]
    pub fn _set_scene_details(details_json: String);

    // Given a JS array of JsLayerInfo structs and an ID for the currently
    // selected layer, this will update the layer info accordion in the bottom
    // right of the scene and the sprite dropdown "Move to layer" option with
    // this collection of layers.
    #[wasm_bindgen(js_name = update_layers_list)]
    fn _update_layers_list(layer_info: Array, selected: Id);

    // Expose closures
    #[wasm_bindgen]
    pub fn expose_closure(name: &str, closure: &Closure<dyn FnMut()>);

    #[wasm_bindgen(js_name = expose_closure)]
    pub fn expose_closure_f64(name: &str, closure: &Closure<dyn FnMut(f64)>);

    #[wasm_bindgen(js_name = expose_closure)]
    pub fn expose_closure_string_out(name: &str, closure: &Closure<dyn FnMut() -> String>);

    #[wasm_bindgen(js_name = expose_closure)]
    pub fn expose_closure_string_in(name: &str, closure: &Closure<dyn FnMut(String)>);

    #[wasm_bindgen(js_name = expose_closure)]
    pub fn expose_closure_f64_string(name: &str, closure: &Closure<dyn FnMut(f64, String)>);

    #[wasm_bindgen(js_name = expose_closure)]
    pub fn expose_closure_f64_bool(name: &str, closure: &Closure<dyn FnMut(f64, bool)>);

    #[wasm_bindgen(js_name = expose_closure)]
    pub fn expose_closure_f64_f64(name: &str, closure: &Closure<dyn FnMut(f64, f64)>);

    #[wasm_bindgen(js_name = expose_closure)]
    pub fn expose_closure_f64x4(name: &str, closure: &Closure<dyn FnMut(f64, f64, f64, f64)>);

    #[wasm_bindgen(js_name = expose_closure)]
    pub fn expose_closure_array(name: &str, closure: &Closure<dyn FnMut() -> Array>);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    pub fn log(s: &str);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    pub fn log_js_value(v: &JsValue);
}

// I want this around for debugging
#[allow(unused_macros)]
macro_rules! flog {
    ($($tts:tt)*) => {
        crate::bridge::log(&format!($($tts)*))
    }
}

#[allow(unused_imports)]
pub(crate) use flog;

pub type Gl = WebGl2RenderingContext;

struct Element {
    element: HtmlElement,
}

impl Element {
    fn new(name: &str) -> anyhow::Result<Element> {
        match create_element(name)?.dyn_into::<HtmlElement>() {
            Ok(e) => Ok(Element { element: e }),
            Err(_) => Err(anyhow::anyhow!("Couldn't cast to HtmlElement.")),
        }
    }

    fn set_css(&self, property: &str, value: &str) -> anyhow::Result<()> {
        self.element
            .style()
            .set_property(property, value)
            .map_err(|e| anyhow::anyhow!("Failed to set element CSS: {e:?}."))
    }

    fn set_attr(&self, name: &str, value: &str) -> anyhow::Result<()> {
        self.element
            .set_attribute(name, value)
            .map_err(|e| anyhow::anyhow!("Failed to set element attribute: {e:?}."))
    }
}

struct Canvas {
    element: Rc<HtmlCanvasElement>,
    gl: Rc<Gl>,

    // Array where MouseEvents are stored to be handled by the core loop.
    events: Rc<Array>,
}

impl Canvas {
    fn new(element: HtmlCanvasElement) -> anyhow::Result<Canvas> {
        let gl = Rc::new(create_context(&element)?);

        Ok(Canvas {
            element: Rc::new(element),
            gl,
            events: Rc::new(Array::new()),
        })
    }

    /// Create a new canvas element and set it up to fill the screen.
    fn new_element() -> anyhow::Result<Canvas> {
        let element = {
            if let Some(e) = get_document()?.get_element_by_id("canvas") {
                e
            } else {
                create_appended("canvas")?
            }
        };

        let canvas = match element.dyn_into::<HtmlCanvasElement>() {
            Ok(c) => Canvas::new(c)?,
            Err(_) => return Err(anyhow::anyhow!("Couldn't cast Element to HtmlCanvas.",)),
        };

        canvas.init()?;

        Ok(canvas)
    }

    /// Set the canvas' dimensions to those of the viewport.
    /// This is static as it's useful to call it from closures
    fn fill_window(canvas: &HtmlCanvasElement) -> anyhow::Result<()> {
        let (vp_w, vp_h) = get_window_dimensions()?;

        canvas.set_width(vp_w);
        canvas.set_height(vp_h);

        Ok(())
    }

    /// Set up the canvas to fill the full screen and resize with the window.
    fn init(&self) -> anyhow::Result<()> {
        self.position_top_left()?;
        self.configure_resize()?;
        self.configure_events()?;
        Canvas::fill_window(&self.element)?;

        Ok(())
    }

    fn set_css(&self, property: &str, value: &str) -> anyhow::Result<()> {
        self.element
            .style()
            .set_property(property, value)
            .map_err(|e| anyhow::anyhow!("Failed to set canvas CSS: {e:?}."))
    }

    /// Set CSS on the canvas element to ensure it fills the screen without
    /// scroll bars.
    fn position_top_left(&self) -> anyhow::Result<()> {
        /*
        {
            left: 0;
            position: absolute;
            top: 0;
        }
        */
        self.set_css("position", "absolute")?;
        self.set_css("top", "0")?;
        self.set_css("left", "0")?;

        Ok(())
    }

    /// Adds an event listener to resize the canvas to fill the window on
    /// viewport resize.
    fn configure_resize(&self) -> anyhow::Result<()> {
        let canvas = self.element.clone();
        let gl = self.gl.clone();
        let closure = Closure::wrap(Box::new(move |_event: UiEvent| {
            Canvas::fill_window(&canvas).ok();
            gl.viewport(0, 0, canvas.width() as i32, canvas.height() as i32);
        }) as Box<dyn FnMut(_)>);

        let result =
            window()?.add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref());
        closure.forget();
        result.map_err(|e| anyhow::anyhow!("Failed to add resize listener: {e:?}."))
    }

    fn configure_events(&self) -> anyhow::Result<()> {
        for event_name in ["mouseenter", "mousemove"] {
            // Grab focus on mouse events
            let element = self.element.clone();
            let listener = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
                element.focus().ok();
            }) as Box<dyn FnMut(web_sys::MouseEvent)>);
            if self
                .element
                .add_event_listener_with_callback(event_name, listener.as_ref().unchecked_ref())
                .is_err()
            {
                return Err(anyhow::anyhow!(
                    "Failed to add mouse event listener to canvas."
                ));
            }
            listener.forget();
        }

        for event_name in [
            "mousedown",
            "mouseup",
            "mouseleave",
            "mousemove",
            "wheel",
            "keydown",
        ] {
            let events = self.events.clone();
            let listener = Closure::wrap(Box::new(move |event: web_sys::UiEvent| {
                events.push(&event);

                if event_name == "wheel" {
                    event.prevent_default();
                }
            }) as Box<dyn FnMut(web_sys::UiEvent)>);

            if self
                .element
                .add_event_listener_with_callback(event_name, listener.as_ref().unchecked_ref())
                .is_err()
            {
                return Err(anyhow::anyhow!("Failed to add event listener to canvas."));
            }

            listener.forget();
        }

        Ok(())
    }

    fn configure_upload(&self, texture_queue: Rc<Array>) -> anyhow::Result<()> {
        let input = Rc::new(create_file_upload()?);
        let result = {
            let c_input = input.clone();
            let closure = Closure::wrap(Box::new(move |_event: web_sys::InputEvent| {
                let file = match c_input.files() {
                    Some(fs) => match fs.get(0) {
                        Some(f) => f,
                        None => return,
                    },
                    None => return,
                };

                let file_reader = match FileReader::new() {
                    Ok(fr) => Rc::new(fr),
                    Err(_) => return,
                };

                // File load handling
                let fr_ref = file_reader.clone();
                let tq_ref = texture_queue.clone();
                let closure = Closure::wrap(Box::new(move |_event: ProgressEvent| {
                    let file = match fr_ref.result() {
                        Ok(f) => f,
                        Err(_) => return,
                    };

                    let array = js_sys::Array::new();
                    array.push(&file);

                    let blob = match Blob::new_with_buffer_source_sequence(&array) {
                        Ok(b) => b,
                        Err(_) => return,
                    };

                    let src = match Url::create_object_url_with_blob(&blob) {
                        Ok(s) => s,
                        Err(_) => return,
                    };

                    let image = match HtmlImageElement::new() {
                        Ok(i) => Rc::new(i),
                        Err(_) => return,
                    };

                    {
                        let im_ref = image.clone();
                        let tq_ref = tq_ref.clone();
                        let closure = Closure::wrap(Box::new(move || {
                            tq_ref.push(&im_ref);
                        }) as Box<dyn FnMut()>);
                        image.set_onload(Some(closure.as_ref().unchecked_ref()));
                        closure.forget();
                    }

                    image.set_src(&src);
                }) as Box<dyn FnMut(_)>);

                if file_reader
                    .add_event_listener_with_callback("loadend", closure.as_ref().unchecked_ref())
                    .is_err()
                {
                    return;
                }
                closure.forget();

                file_reader.read_as_array_buffer(&file).ok();
            }) as Box<dyn FnMut(_)>);
            let result =
                input.add_event_listener_with_callback("input", closure.as_ref().unchecked_ref());
            closure.forget();
            result
        };

        match result {
            Ok(()) => (),
            Err(_) => return Err(anyhow::anyhow!("Failed to add event listener.")),
        };

        {
            let closure = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
                input.click();
            }) as Box<dyn FnMut(_)>);
            let result = self
                .element
                .add_event_listener_with_callback("auxclick", closure.as_ref().unchecked_ref());
            closure.forget();
            result
        }
        .map_err(|e| anyhow::anyhow!("Failed to add click listener: {e:?}."))
    }
}

#[derive(Debug)]
pub enum MouseAction {
    Down,
    Up,
    Leave,
    Move,
    Wheel(f32),
}

#[derive(Clone, Copy, Debug)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    Back,
    Forward,
    Unknown,
}

impl MouseButton {
    fn from(button: i16) -> Self {
        // Reference:
        // https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/button
        match button {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            3 => MouseButton::Back,
            4 => MouseButton::Forward,
            _ => MouseButton::Unknown,
        }
    }
}

#[derive(Debug)]
pub enum KeyboardAction {
    Down,
    Up,
}

#[derive(Clone, Copy, Debug)]
pub enum Key {
    Alt,
    Control,
    Delete,
    Equals,
    Down,
    Escape,
    Left,
    Minus,
    Plus,
    Right,
    Shift,
    Space,
    Underscore,
    Up,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    X,
    Y,
    Z,
    Unknown,
}

impl Key {
    const LOG_UNKNOWN: bool = true;

    fn from(key: &str) -> Self {
        match key {
            "Alt" => Self::Alt,
            "Control" => Self::Control,
            "Delete" => Self::Delete,
            "Escape" => Self::Escape,
            "Shift" => Self::Shift,
            "ArrowDown" => Self::Down,
            "ArrowLeft" => Self::Left,
            "ArrowRight" => Self::Right,
            "ArrowUp" => Self::Up,
            "-" => Self::Minus,
            "_" => Self::Underscore,
            "=" => Self::Equals,
            "+" => Self::Plus,
            " " => Self::Space,
            "a" => Self::A,
            "b" => Self::B,
            "c" => Self::C,
            "d" => Self::D,
            "e" => Self::E,
            "f" => Self::F,
            "g" => Self::G,
            "h" => Self::H,
            "i" => Self::I,
            "j" => Self::J,
            "k" => Self::K,
            "l" => Self::L,
            "m" => Self::M,
            "n" => Self::N,
            "o" => Self::O,
            "p" => Self::P,
            "q" => Self::Q,
            "r" => Self::R,
            "s" => Self::S,
            "t" => Self::T,
            "u" => Self::U,
            "v" => Self::V,
            "x" => Self::X,
            "y" => Self::Y,
            "z" => Self::Z,
            "A" => Self::A,
            "B" => Self::B,
            "C" => Self::C,
            "D" => Self::D,
            "E" => Self::E,
            "F" => Self::F,
            "G" => Self::G,
            "H" => Self::H,
            "I" => Self::I,
            "J" => Self::J,
            "K" => Self::K,
            "L" => Self::L,
            "M" => Self::M,
            "N" => Self::N,
            "O" => Self::O,
            "P" => Self::P,
            "Q" => Self::Q,
            "R" => Self::R,
            "S" => Self::S,
            "T" => Self::T,
            "U" => Self::U,
            "V" => Self::V,
            "X" => Self::X,
            "Y" => Self::Y,
            "Z" => Self::Z,
            _ => {
                if Self::LOG_UNKNOWN {
                    flog!("Unknown key: {key}");
                }
                Self::Unknown
            }
        }
    }

    pub fn is_arrow(&self) -> bool {
        matches!(self, Key::Down | Key::Left | Key::Right | Key::Up)
    }
}

#[derive(Debug)]
pub enum Input {
    Mouse(ViewportPoint, MouseAction, MouseButton),
    Keyboard(KeyboardAction, Key),
}

#[derive(Debug)]
pub struct InputEvent {
    pub input: Input,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl InputEvent {
    fn from_web_sys(event: &web_sys::UiEvent) -> Option<InputEvent> {
        match event.type_().as_str() {
            "keydown" | "keyup" => {
                Self::from_keyboard(event.unchecked_ref::<web_sys::KeyboardEvent>())
            }
            "mousedown" | "mouseleave" | "mousemove" | "mouseup" | "wheel" => {
                Self::from_mouse(event.unchecked_ref::<web_sys::MouseEvent>())
            }
            _ => None,
        }
    }

    fn from_mouse(event: &web_sys::MouseEvent) -> Option<InputEvent> {
        let action = match event.type_().as_str() {
            "mousedown" => MouseAction::Down,
            "mouseleave" => MouseAction::Leave,
            "mousemove" => MouseAction::Move,
            "mouseup" => MouseAction::Up,
            "wheel" => {
                let event = event.unchecked_ref::<web_sys::WheelEvent>();

                // Because the app never has scroll bars, the delta is always
                // reported in the y
                MouseAction::Wheel(event.delta_y() as f32)
            }
            _ => return None,
        };

        Some(InputEvent {
            input: Input::Mouse(
                ViewportPoint::new(event.x(), event.y()),
                action,
                MouseButton::from(event.button()),
            ),
            shift: event.shift_key(),
            ctrl: event.ctrl_key(),
            alt: event.alt_key(),
        })
    }

    fn from_keyboard(event: &web_sys::KeyboardEvent) -> Option<InputEvent> {
        let action = match event.type_().as_str() {
            "keydown" => KeyboardAction::Down,
            "keyup" => KeyboardAction::Up,
            _ => return None,
        };

        Some(InputEvent {
            input: Input::Keyboard(action, Key::from(&event.key())),
            shift: event.shift_key(),
            ctrl: event.ctrl_key(),
            alt: event.alt_key(),
        })
    }
}

pub struct Context {
    // WebGL context. Wrapped in Rc because various structs and closures want
    // for references to it.
    gl: Rc<Gl>,

    // Holds information about the HTML canvas associated with the WebGL
    // context.
    canvas: Canvas,

    // Wrapper around OpenGL Rendering functions
    renderer: Renderer,

    // A JS Array which the front end pushes uploaded images to. The Context
    // then loads any images waiting in the queue before rendering each frame.
    // Wrapped in Rc such that it can be accessed from a closure passed to JS.
    texture_queue: Rc<Array>,
}

impl Context {
    pub fn new() -> anyhow::Result<Context> {
        let canvas = Canvas::new_element()?;
        let renderer = Renderer::new(canvas.gl.clone())?;
        let ctx = Context {
            gl: canvas.gl.clone(),
            canvas,
            renderer,
            texture_queue: Rc::new(get_texture_queue()),
        };

        Ok(ctx)
    }

    fn configure_upload(&self) {
        self.canvas
            .configure_upload(self.texture_queue.clone())
            .ok();
    }

    pub fn viewport_size(&self) -> (u32, u32) {
        (self.canvas.element.width(), self.canvas.element.height())
    }

    pub fn events(&self) -> Option<Vec<InputEvent>> {
        if self.canvas.events.length() == 0 {
            return None;
        }

        let mut events = Vec::new();
        while self.canvas.events.length() > 0 {
            let event = self.canvas.events.pop();
            let event = event.unchecked_ref::<web_sys::UiEvent>();
            if let Some(e) = InputEvent::from_web_sys(event) {
                events.push(e);
            };
        }

        match events.len() {
            0 => None,
            _ => Some(events),
        }
    }

    // Returns true if new textures were loaded.
    pub fn load_texture_queue(&mut self) -> bool {
        if self.texture_queue.length() == 0 {
            return false;
        }

        while self.texture_queue.length() > 0 {
            let img = self.texture_queue.pop();

            // Cast the img to a HTMLImageElement; this array will only contain
            // such elements, so this cast is safe.
            let img = img.unchecked_ref::<HtmlImageElement>();
            self.renderer.load_image(img);
        }
        true
    }

    pub fn clear(&self, vp: Rect) {
        self.gl.viewport(0, 0, vp.w as i32, vp.h as i32);
        self.gl.clear(Gl::COLOR_BUFFER_BIT);
    }

    pub fn draw_grid(&mut self, vp: Rect, dims: Rect, grid_size: f32) {
        self.renderer.render_grid(vp, dims, grid_size);
    }

    pub fn draw_sprites(&mut self, vp: Rect, sprites: &[Sprite], grid_size: f32) {
        for sprite in sprites.iter() {
            self.renderer.draw_sprite(sprite, vp, grid_size);
        }
    }

    pub fn draw_outline(&mut self, vp: Rect, outline: Rect) {
        self.renderer.draw_outline(vp, outline);
    }

    pub fn set_cursor(&self, cursor: Cursor) {
        cursor.set_for(&self.canvas.element).ok();
    }
}

pub fn set_selected_sprite(sprite: SpriteDetails) {
    if let Ok(sprite_json) = serde_json::ser::to_string(&sprite) {
        _set_selected_sprite(sprite_json);
    }
}

pub fn set_scene_details(details: SceneDetails) {
    if let Ok(details_json) = serde_json::to_string(&details) {
        _set_scene_details(details_json);
    }
}

#[derive(Deserialize, Serialize)]
pub struct JsLayerInfo {
    pub id: Id,
    pub title: String,
    pub z: f64,
    pub visible: bool,
    pub locked: bool,
    pub n_sprites: f64,
}

impl JsLayerInfo {
    fn from(layer: &Layer) -> Self {
        JsLayerInfo {
            id: layer.id,
            title: layer.title.clone(),
            z: layer.z as f64,
            visible: layer.visible,
            locked: layer.locked,
            n_sprites: layer.sprites.len() as f64,
        }
    }

    fn js_value_from(layer: &Layer) -> JsValue {
        // Safe to unwrap as this type is known to be deserialisable.
        JsValue::from_serde(&Self::from(layer)).unwrap()
    }
}

fn layer_info(layers: &[Layer]) -> Array {
    layers.iter().map(JsLayerInfo::js_value_from).collect()
}

pub fn update_layers_list(layers: &[Layer]) {
    _update_layers_list(layer_info(layers), layers.get(0).map(|l| l.id).unwrap_or(0));
}

#[derive(Debug)]
pub enum Cursor {
    // General
    Auto,
    Default,
    None,
    // Links and status
    ContextMenu,
    Help,
    Pointer,
    Progress,
    Wait,
    // Selection
    Cell,
    Crosshair,
    Text,
    VerticalText,
    // Drag and drop
    Alias,
    Copy,
    Move,
    NoDrop,
    NotAllowed,
    Grab,
    Grabbing,
    // Resizing and scrolling
    AltScroll,
    ColResize,
    RowResize,
    NResize,
    EResize,
    SResize,
    WResize,
    NeResize,
    NwResize,
    SeResize,
    SwResize,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
    // Zooming
    ZoomIn,
    ZoomOut,
}

impl Cursor {
    /// Returns the css string for this cursor.
    fn css(&self) -> String {
        let mut css = String::new();
        for c in format!("{self:?}").chars() {
            if c.is_uppercase() {
                if !css.is_empty() {
                    css.push('-');
                }
                css.push(c.to_ascii_lowercase());
            } else {
                css.push(c);
            }
        }
        css
    }

    fn set_for(&self, element: &HtmlElement) -> anyhow::Result<()> {
        element
            .style()
            .set_property("cursor", &self.css())
            .map_err(|e| anyhow::anyhow!("Error: {e:?}"))
    }

    #[must_use]
    pub fn override_default(self, other: Self) -> Self {
        if matches!(self, Self::Default) {
            other
        } else {
            self
        }
    }
}

fn create_context(element: &HtmlCanvasElement) -> anyhow::Result<Gl> {
    let gl = match element.get_context("webgl2") {
        Ok(Some(c)) => match c.dyn_into::<Gl>() {
            Ok(c) => c,
            Err(_) => return Err(anyhow::anyhow!("Failed to cast to WebGL context.",)),
        },
        _ => return Err(anyhow::anyhow!("Failed to get rendering context.")),
    };

    // Enable transparency
    gl.enable(Gl::BLEND);
    gl.blend_func(Gl::SRC_ALPHA, Gl::ONE_MINUS_SRC_ALPHA);

    Ok(gl)
}

fn window() -> anyhow::Result<Window> {
    match web_sys::window() {
        Some(w) => Ok(w),
        None => Err(anyhow::anyhow!("No Window.")),
    }
}

fn get_document() -> anyhow::Result<Document> {
    match window()?.document() {
        Some(d) => Ok(d),
        None => Err(anyhow::anyhow!("No Document.")),
    }
}

fn get_body() -> anyhow::Result<HtmlElement> {
    match get_document()?.body() {
        Some(b) => Ok(b),
        None => Err(anyhow::anyhow!("No Body.")),
    }
}

pub fn websocket_url() -> anyhow::Result<Option<String>> {
    let win = match window() {
        Ok(w) => w,
        Err(_) => return Err(anyhow::anyhow!("Failed to read window Location.")),
    };
    let loc = win.location();
    let host = match loc.host() {
        Ok(h) => h,
        Err(_) => return Err(anyhow::anyhow!("Failed to read window host.")),
    };

    match loc.pathname() {
        Ok(path) => {
            let mut parts = path.split('/').collect::<Vec<&str>>();
            parts.retain(|p| !p.is_empty());
            match parts[..] {
                ["game", game_key, "client", client_key] => Ok(Some(format!(
                    "ws://{}/game/{}/{}",
                    &host, game_key, client_key
                ))),
                _ => Ok(None),
            }
        }
        Err(_) => Err(anyhow::anyhow!("Failed to read window pathname.")),
    }
}

fn create_element(name: &str) -> anyhow::Result<web_sys::Element> {
    get_document()?
        .create_element(name)
        .map_err(|e| anyhow::anyhow!("Element creation failed: {e:?}."))
}

fn create_appended(name: &str) -> anyhow::Result<web_sys::Element> {
    let element = create_element(name)?;
    match get_body()?.append_child(&element) {
        Ok(_) => Ok(element),
        Err(_) => Err(anyhow::anyhow!("Failed to append element.")),
    }
}

fn get_window_dimensions() -> anyhow::Result<(u32, u32)> {
    let win = window()?;

    match (win.inner_width(), win.inner_height()) {
        (Ok(w), Ok(h)) => match (w.as_f64(), h.as_f64()) {
            (Some(w), Some(h)) => Ok((w as u32, h as u32)),
            _ => Err(anyhow::anyhow!("Window dimensions non-numeric.")),
        },
        _ => Err(anyhow::anyhow!("No Window dimensions.")),
    }
}

fn create_file_upload() -> anyhow::Result<HtmlInputElement> {
    let element = Element::new("input")?;

    element.set_attr("type", "file")?;
    element.set_attr("accept", "image/*")?;

    element
        .element
        .dyn_into::<HtmlInputElement>()
        .map_err(|_| anyhow::anyhow!("Failed to cast element to HtmlInputElement."))
}

pub fn request_animation_frame(f: &Closure<dyn FnMut()>) -> anyhow::Result<()> {
    match window()?.request_animation_frame(f.as_ref().unchecked_ref()) {
        Ok(_) => Ok(()),
        Err(_) => Err(anyhow::anyhow!("Failed to get animation frame.")),
    }
}

fn set_visible(id: &str, visible: bool) -> anyhow::Result<()> {
    if let Some(element) = get_document()?.get_element_by_id(id) {
        element
            .unchecked_ref::<HtmlElement>()
            .style()
            .set_property("display", if visible { "" } else { "none" })
            .ok();
    }
    Ok(())
}

fn set_checked(id: &str) -> anyhow::Result<()> {
    if let Some(element) = get_document()?.get_element_by_id(id) {
        element
            .unchecked_ref::<HtmlInputElement>()
            .set_checked(true);
    }

    Ok(())
}

fn enum_to_id(tool: impl serde::Serialize, pref: &str) -> anyhow::Result<String> {
    if let Ok(s) = serde_json::ser::to_string(&tool) {
        Ok(format!("{pref}{}", s.to_lowercase().replace('"', "")))
    } else {
        Err(anyhow::anyhow!("Serialisation error."))
    }
}

pub fn set_active_draw_tool(tool: impl serde::Serialize) -> anyhow::Result<()> {
    const ID_PREFIX: &str = "draw_radio_";
    set_checked(&enum_to_id(tool, ID_PREFIX)?)
}

pub fn set_active_tool(tool: crate::viewport::Tool) -> anyhow::Result<()> {
    // Note: this needs to match web/include/scene/menu/tools/tools_menu.html
    const ID_PREFIX: &str = "tool_radio_";
    set_checked(&enum_to_id(tool, ID_PREFIX)?)?;
    set_visible("draw_menu", matches!(tool, crate::viewport::Tool::Draw))?;
    Ok(())
}

pub fn set_role(role: scene::perms::Role) {
    set_visible("canvas_menu", !role.spectator()).ok();
    set_visible("tools_menu", role.player()).ok();
    set_visible("sprite_menu", role.player()).ok();
    set_visible("layers_menu", role.editor()).ok();
    set_visible("scene_menu", role.editor()).ok();
    set_visible("show_offcanvas", role.editor()).ok();
}
