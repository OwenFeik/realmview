use js_sys::{Array, Float32Array};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    Blob, Document, FileReader, HtmlCanvasElement, HtmlElement, HtmlImageElement, HtmlInputElement,
    InputEvent, ProgressEvent, UiEvent, Url, WebGl2RenderingContext, Window,
};

use scene::{comms::SceneEvent, Id, Rect, Sprite};

use crate::programs::Renderer;
use crate::viewport::ViewportPoint;

#[wasm_bindgen]
extern "C" {
    pub fn get_texture_queue() -> Array;
    pub fn get_event_queue() -> Array;
    pub fn load_texture(id: Id);

    #[wasm_bindgen(js_name = expose_closure)]
    pub fn expose_closure_f64(name: &str, closure: &Closure<dyn FnMut(f64)>);

    #[wasm_bindgen(js_name = expose_closure)]
    pub fn expose_closure_string(name: &str, closure: &Closure<dyn FnMut() -> String>);

    #[wasm_bindgen(js_name = expose_closure)]
    pub fn expose_closure_string_string(name: &str, closure: &Closure<dyn FnMut(String) -> String>);

    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    pub fn log_bool(b: bool);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    pub fn log_int(i: i32);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    pub fn log_js_value(v: &JsValue);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    pub fn log_float(f: f32);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    pub fn log_arr(a: Float32Array);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    pub fn log_image(i: &HtmlImageElement);
}

pub type Gl = WebGl2RenderingContext;

#[derive(Debug)]
pub enum JsError {
    ResourceError(&'static str),
    TypeError(&'static str),
}

struct Element {
    element: HtmlElement,
}

impl Element {
    fn new(name: &str) -> Result<Element, JsError> {
        match create_element(name)?.dyn_into::<HtmlElement>() {
            Ok(e) => Ok(Element { element: e }),
            Err(_) => Err(JsError::TypeError("Couldn't cast to HtmlElement.")),
        }
    }

    fn set_css(&self, property: &str, value: &str) -> Result<(), JsError> {
        self.element
            .style()
            .set_property(property, value)
            .or(Err(JsError::ResourceError("Failed to set element CSS.")))
    }

    fn set_attr(&self, name: &str, value: &str) -> Result<(), JsError> {
        self.element
            .set_attribute(name, value)
            .or(Err(JsError::ResourceError(
                "Failed to set element attribute.",
            )))
    }
}

struct Canvas {
    element: Rc<HtmlCanvasElement>,
    gl: Rc<Gl>,

    // Array where MouseEvents are stored to be handled by the core loop.
    events: Rc<Array>,
}

impl Canvas {
    fn new(element: HtmlCanvasElement) -> Result<Canvas, JsError> {
        let gl = Rc::new(create_context(&element)?);

        Ok(Canvas {
            element: Rc::new(element),
            gl,
            events: Rc::new(Array::new()),
        })
    }

    /// Create a new canvas element and set it up to fill the screen.
    fn new_element() -> Result<Canvas, JsError> {
        let element = {
            if let Some(e) = get_document()?.get_element_by_id("canvas") {
                e
            } else {
                create_appended("canvas")?
            }
        };

        let canvas = match element.dyn_into::<HtmlCanvasElement>() {
            Ok(c) => Canvas::new(c)?,
            Err(_) => return Err(JsError::TypeError("Couldn't cast Element to HtmlCanvas.")),
        };

        canvas.init()?;

        Ok(canvas)
    }

    /// Set the canvas' dimensions to those of the viewport.
    /// This is static as it's useful to call it from closures
    fn fill_window(canvas: &HtmlCanvasElement) -> Result<(), JsError> {
        let (vp_w, vp_h) = get_window_dimensions()?;

        canvas.set_width(vp_w);
        canvas.set_height(vp_h);

        Ok(())
    }

    /// Set up the canvas to fill the full screen and resize with the window.
    fn init(&self) -> Result<(), JsError> {
        self.position_top_left()?;
        self.configure_resize()?;
        self.configure_events()?;
        Canvas::fill_window(&self.element)?;

        Ok(())
    }

    fn set_css(&self, property: &str, value: &str) -> Result<(), JsError> {
        self.element
            .style()
            .set_property(property, value)
            .or(Err(JsError::ResourceError("Failed to set canvas CSS.")))
    }

    /// Set CSS on the canvas element to ensure it fills the screen without
    /// scroll bars.
    fn position_top_left(&self) -> Result<(), JsError> {
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
    fn configure_resize(&self) -> Result<(), JsError> {
        let canvas = self.element.clone();
        let gl = self.gl.clone();
        let closure = Closure::wrap(Box::new(move |_event: UiEvent| {
            Canvas::fill_window(&canvas).ok();
            gl.viewport(0, 0, canvas.width() as i32, canvas.height() as i32);
        }) as Box<dyn FnMut(_)>);

        let result =
            window()?.add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref());
        closure.forget();
        result.or(Err(JsError::ResourceError(
            "Failed to add resize listener.",
        )))
    }

    fn configure_events(&self) -> Result<(), JsError> {
        for event_name in vec!["mousedown", "mouseup", "mouseleave", "mousemove", "wheel"].iter() {
            let events = self.events.clone();
            let listener = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
                event.prevent_default();
                events.push(&event);
            }) as Box<dyn FnMut(web_sys::MouseEvent)>);

            match self
                .element
                .add_event_listener_with_callback(event_name, listener.as_ref().unchecked_ref())
            {
                Ok(_) => (),
                Err(_) => {
                    return Err(JsError::ResourceError(
                        "Failed to add mouse event listener to canvas.",
                    ))
                }
            };

            listener.forget();
        }

        Ok(())
    }

    fn configure_upload(&self, texture_queue: Rc<Array>) -> Result<(), JsError> {
        let input = Rc::new(create_file_upload()?);
        let result = {
            let c_input = input.clone();
            let closure = Closure::wrap(Box::new(move |_event: InputEvent| {
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
            Err(_) => return Err(JsError::ResourceError("Failed to add event listener.")),
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
        .or(Err(JsError::ResourceError("Failed to add click listener.")))
    }
}

pub enum EventType {
    MouseDown,
    MouseUp,
    MouseLeave,
    MouseMove,
    MouseWheel(f32),
}

pub struct MouseEvent {
    pub at: ViewportPoint,
    pub event_type: EventType,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl MouseEvent {
    fn from_web_sys(event: &web_sys::MouseEvent) -> Option<MouseEvent> {
        let event_type = match event.type_().as_str() {
            "mousedown" => EventType::MouseDown,
            "mouseleave" => EventType::MouseLeave,
            "mousemove" => EventType::MouseMove,
            "mouseup" => EventType::MouseUp,
            "wheel" => {
                let event = event.unchecked_ref::<web_sys::WheelEvent>();

                // Because the app never has scroll bars, the delta is always reported in the y
                EventType::MouseWheel(event.delta_y() as f32)
            }
            _ => return None,
        };

        Some(MouseEvent {
            at: ViewportPoint::new(event.x(), event.y()),
            event_type,
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

    // A JS Array which the front end pushes events to. At this stage such
    // events are just sprite additions.
    event_queue: Rc<Array>,
}

impl Context {
    pub fn new() -> Result<Context, JsError> {
        let canvas = Canvas::new_element()?;
        let renderer = Renderer::new(canvas.gl.clone())?;
        let ctx = Context {
            gl: canvas.gl.clone(),
            canvas,
            renderer,
            texture_queue: Rc::new(get_texture_queue()),
            event_queue: Rc::new(get_event_queue()),
        };
        ctx.configure_upload();

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

    pub fn events(&self) -> Option<Vec<MouseEvent>> {
        if self.canvas.events.length() == 0 {
            return None;
        }

        let mut events = Vec::new();
        while self.canvas.events.length() > 0 {
            let event = self.canvas.events.pop();
            let event = event.unchecked_ref::<web_sys::MouseEvent>();
            if let Some(e) = MouseEvent::from_web_sys(event) {
                events.push(e);
            };
        }

        match events.len() {
            0 => None,
            _ => Some(events),
        }
    }

    pub fn load_queue(&mut self) -> Option<Vec<SceneEvent>> {
        if self.texture_queue.length() == 0 && self.event_queue.length() == 0 {
            return None;
        }

        let mut events = Vec::new();

        if self.texture_queue.length() > 0 {
            while self.texture_queue.length() > 0 {
                let img = self.texture_queue.pop();

                // Cast the img to a HTMLImageElement; this array will only contain
                // such elements, so this cast is safe.
                let img = img.unchecked_ref::<HtmlImageElement>();
                self.renderer.load_image(img);
            }

            events.push(SceneEvent::Dummy);
        }

        if self.event_queue.length() > 0 {
            while self.event_queue.length() > 0 {
                if let Some(id) = self.event_queue.pop().as_f64() {
                    let id = id as i64;
                    events.push(SceneEvent::SpriteNew(Sprite::new(id), 0));
                }
            }
        }

        Some(events)
    }

    pub fn clear(&self, vp: Rect) {
        self.gl.viewport(0, 0, vp.w as i32, vp.h as i32);
        self.gl.clear(Gl::COLOR_BUFFER_BIT);
    }

    pub fn draw_grid(&mut self, vp: Rect, grid_size: f32) {
        self.renderer.render_grid(vp, grid_size);
    }

    pub fn draw_sprites(&mut self, vp: Rect, sprites: &[Sprite], grid_size: f32) {
        for sprite in sprites.iter() {
            self.renderer.draw_texture(
                vp,
                sprite.texture,
                Rect::scaled_from(sprite.rect, grid_size),
            );
        }
    }

    pub fn draw_outline(&mut self, vp: Rect, outline: Rect) {
        self.renderer.draw_outline(vp, outline);
    }
}

fn create_context(element: &HtmlCanvasElement) -> Result<Gl, JsError> {
    let gl = match element.get_context("webgl2") {
        Ok(Some(c)) => match c.dyn_into::<Gl>() {
            Ok(c) => c,
            Err(_) => return Err(JsError::TypeError("Failed to cast to WebGL context.")),
        },
        _ => return Err(JsError::ResourceError("Failed to get rendering context.")),
    };

    // Enable transparency
    gl.enable(Gl::BLEND);
    gl.blend_func(Gl::SRC_ALPHA, Gl::ONE_MINUS_SRC_ALPHA);

    Ok(gl)
}

fn window() -> Result<Window, JsError> {
    match web_sys::window() {
        Some(w) => Ok(w),
        None => Err(JsError::ResourceError("No Window.")),
    }
}

fn get_document() -> Result<Document, JsError> {
    match window()?.document() {
        Some(d) => Ok(d),
        None => Err(JsError::ResourceError("No Document.")),
    }
}

fn get_body() -> Result<HtmlElement, JsError> {
    match get_document()?.body() {
        Some(b) => Ok(b),
        None => Err(JsError::ResourceError("No Body.")),
    }
}

pub fn websocket_url() -> Result<Option<String>, JsError> {
    let win = match window() {
        Ok(w) => w,
        Err(_) => return Err(JsError::ResourceError("Failed to read window Location.")),
    };
    let loc = win.location();
    let host = match loc.host() {
        Ok(h) => h,
        Err(_) => return Err(JsError::ResourceError("Failed to read window host.")),
    };

    match loc.pathname() {
        Ok(path) => {
            let mut parts = path.split("/").collect::<Vec<&str>>();
            parts.retain(|p| p.len() != 0);
            match parts[..] {
                ["game", game_key, "client", client_key] => Ok(Some(format!(
                    "ws://{}/game/{}/{}",
                    &host, game_key, client_key
                ))),
                _ => Ok(None),
            }
        }
        Err(_) => Err(JsError::ResourceError("Failed to read window pathname.")),
    }
}

fn create_element(name: &str) -> Result<web_sys::Element, JsError> {
    get_document()?
        .create_element(name)
        .or(Err(JsError::ResourceError("Element creation failed.")))
}

fn create_appended(name: &str) -> Result<web_sys::Element, JsError> {
    let element = create_element(name)?;
    match get_body()?.append_child(&element) {
        Ok(_) => Ok(element),
        Err(_) => Err(JsError::ResourceError("Failed to append element.")),
    }
}

fn get_window_dimensions() -> Result<(u32, u32), JsError> {
    let win = window()?;

    match (win.inner_width(), win.inner_height()) {
        (Ok(w), Ok(h)) => match (w.as_f64(), h.as_f64()) {
            (Some(w), Some(h)) => Ok((w as u32, h as u32)),
            _ => Err(JsError::TypeError("Window dimensions non-numeric.")),
        },
        _ => Err(JsError::ResourceError("No Window dimensions.")),
    }
}

fn create_file_upload() -> Result<HtmlInputElement, JsError> {
    let element = Element::new("input")?;

    element.set_attr("type", "file")?;
    element.set_attr("accept", "image/*")?;

    element
        .element
        .dyn_into::<HtmlInputElement>()
        .or(Err(JsError::TypeError(
            "Failed to cast element to HtmlInputElement.",
        )))
}

pub fn request_animation_frame(f: &Closure<dyn FnMut()>) -> Result<(), JsError> {
    match window()?.request_animation_frame(f.as_ref().unchecked_ref()) {
        Ok(_) => Ok(()),
        Err(_) => Err(JsError::ResourceError("Failed to get animation frame.")),
    }
}
