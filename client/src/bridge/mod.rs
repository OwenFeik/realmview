use std::rc::Rc;
use std::sync::atomic::AtomicBool;

use js_sys::Array;
use js_sys::Promise;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::Headers;
use web_sys::Response;
use web_sys::{
    Blob, Document, FileReader, HtmlCanvasElement, HtmlElement, HtmlImageElement, HtmlInputElement,
    ProgressEvent, Request, RequestInit, UiEvent, Url, WebGl2RenderingContext, Window,
};

use crate::dom::element::Element;
use crate::err;
use crate::render::Gl;
use crate::render::WebGlRenderer;
use crate::Res;

#[wasm_bindgen]
extern "C" {
    // Returns an array where loaded texture images will be placed once ready.
    fn get_texture_queue() -> Array;

    // Returns a WebGL2 Context with approriate options set.
    fn get_webgl2_context(element: &HtmlCanvasElement) -> WebGl2RenderingContext;

    // Causes the texture with this ID to be loaded as an image and added to
    // the texture queue once ready.
    pub fn load_texture(media_key: String);

    // Shows or hides the relevant UI elements given a role integer.
    pub fn update_interface(role: i32);

    // Load and set as active scene by scene key
    #[wasm_bindgen]
    pub fn set_active_scene(scene_key: &str);

    // Expose closures
    #[wasm_bindgen]
    pub fn expose_closure(name: &str, closure: &Closure<dyn FnMut()>);

    #[wasm_bindgen(js_name = expose_closure)]
    pub fn expose_closure_f64(name: &str, closure: &Closure<dyn FnMut(f64)>);

    #[wasm_bindgen(js_name = expose_closure)]
    pub fn expose_closure_string_in(name: &str, closure: &Closure<dyn FnMut(String)>);

    #[wasm_bindgen(js_name = expose_closure)]
    pub fn expose_closure_f64x2_string(name: &str, closure: &Closure<dyn FnMut(f64, f64, String)>);

    #[wasm_bindgen(js_name = expose_closure)]
    pub fn expose_closure_f64x4(name: &str, closure: &Closure<dyn FnMut(f64, f64, f64, f64)>);

    #[wasm_bindgen(js_name = expose_closure)]
    pub fn expose_closure_array(name: &str, closure: &Closure<dyn FnMut() -> Array>);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    pub fn console_log(s: &str);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    pub fn log_js_value(v: &JsValue);
}

pub mod event;

// I want this around for debugging
#[allow(unused_macros)]
macro_rules! log {
    ($($tts:tt)*) => {
        crate::bridge::console_log(&format!($($tts)*))
    }
}

#[allow(unused_imports)]
pub(crate) use log;

struct Canvas {
    element: Rc<HtmlCanvasElement>,
    gl: Rc<Gl>,

    // Array where MouseEvents are stored to be handled by the core loop.
    events: Rc<Array>,
}

impl Canvas {
    fn new(element: HtmlCanvasElement) -> Res<Canvas> {
        let gl = Rc::new(create_context(&element)?);

        Ok(Canvas {
            element: Rc::new(element),
            gl,
            events: Rc::new(Array::new()),
        })
    }

    /// Create a new canvas element and set it up to fill the screen.
    fn new_element() -> Res<Canvas> {
        let element = Element::by_id("canvas")
            .unwrap_or_else(|| Element::new("canvas").on_page())
            .raw();
        let canvas = match element.dyn_into::<HtmlCanvasElement>() {
            Ok(c) => Canvas::new(c)?,
            Err(_) => return err("Couldn't cast Element to HtmlCanvas."),
        };

        canvas.init()?;

        Ok(canvas)
    }

    /// Set the canvas' dimensions to those of the viewport.
    /// This is static as it's useful to call it from closures
    fn fill_window(canvas: &HtmlCanvasElement) -> Res<()> {
        let (vp_w, vp_h) = get_window_dimensions()?;

        canvas.set_width(vp_w);
        canvas.set_height(vp_h);

        Ok(())
    }

    /// Set up the canvas to fill the full screen and resize with the window.
    fn init(&self) -> Res<()> {
        self.position_top_left()?;
        self.configure_resize()?;
        self.configure_events()?;
        Canvas::fill_window(&self.element)?;

        Ok(())
    }

    fn set_css(&self, property: &str, value: &str) -> Res<()> {
        self.element
            .style()
            .set_property(property, value)
            .map_err(|e| format!("Failed to set canvas CSS: {e:?}."))
    }

    /// Set CSS on the canvas element to ensure it fills the screen without
    /// scroll bars.
    fn position_top_left(&self) -> Res<()> {
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
    fn configure_resize(&self) -> Res<()> {
        let canvas = self.element.clone();
        let gl = self.gl.clone();
        let closure = Closure::wrap(Box::new(move |_event: UiEvent| {
            Canvas::fill_window(&canvas).ok();
            gl.viewport(0, 0, canvas.width() as i32, canvas.height() as i32);
        }) as Box<dyn FnMut(_)>);

        let result =
            window()?.add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref());
        closure.forget();
        result.map_err(|e| format!("Failed to add resize listener: {e:?}."))
    }

    fn configure_events(&self) -> Res<()> {
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
                return err("Failed to add mouse event listener to canvas.");
            }
            listener.forget();
        }

        for event_name in [
            "mousedown",
            "mouseup",
            "mouseenter",
            "mouseleave",
            "mousemove",
            "wheel",
            "keydown",
            "keyup",
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
                return err("Failed to add event listener to canvas.");
            }

            listener.forget();
        }

        Ok(())
    }

    fn configure_upload(&self, texture_queue: Rc<Array>) -> Res<()> {
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
            Err(_) => return err("Failed to add event listener."),
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
        .map_err(|e| format!("Failed to add click listener: {e:?}."))
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
    renderer: WebGlRenderer,

    // A JS Array which the front end pushes uploaded images to. The Context
    // then loads any images waiting in the queue before rendering each frame.
    // Wrapped in Rc such that it can be accessed from a closure passed to JS.
    texture_queue: Rc<Array>,
}

impl Context {
    pub fn new() -> Res<Context> {
        let canvas = Canvas::new_element()?;
        let renderer = WebGlRenderer::new(canvas.gl.clone())?;
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

    pub fn events(&self) -> Option<Vec<event::InputEvent>> {
        if self.canvas.events.length() == 0 {
            return None;
        }

        let mut events = Vec::new();
        while self.canvas.events.length() > 0 {
            let event = self.canvas.events.pop();
            let event = event.unchecked_ref::<web_sys::UiEvent>();
            if let Some(e) = event::InputEvent::from_web_sys(event) {
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

    pub fn set_cursor(&self, cursor: Cursor) {
        cursor.set_for(&self.canvas.element).ok();
    }

    pub fn renderer(&mut self) -> &mut WebGlRenderer {
        &mut self.renderer
    }
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

    fn set_for(&self, element: &HtmlElement) -> Res<()> {
        element
            .style()
            .set_property("cursor", &self.css())
            .map_err(|e| format!("Error: {e:?}"))
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

fn create_context(element: &HtmlCanvasElement) -> Res<Gl> {
    let gl = get_webgl2_context(element);

    // Enable transparency
    gl.enable(Gl::BLEND);
    gl.blend_func(Gl::SRC_ALPHA, Gl::ONE_MINUS_SRC_ALPHA);

    Ok(gl)
}

fn window() -> Res<Window> {
    match web_sys::window() {
        Some(w) => Ok(w),
        None => err("No Window."),
    }
}

pub fn get_document() -> Res<Document> {
    match window()?.document() {
        Some(d) => Ok(d),
        None => err("No Document."),
    }
}

pub fn get_body() -> Res<HtmlElement> {
    match get_document()?.body() {
        Some(b) => Ok(b),
        None => err("No Body."),
    }
}

fn append_child(parent: &HtmlElement, child: &HtmlElement) {
    parent
        .append_child(child.unchecked_ref::<web_sys::Node>())
        .ok();
}

fn prepend_child(parent: &HtmlElement, child: &HtmlElement) {
    parent
        .insert_before(
            child.unchecked_ref::<web_sys::Node>(),
            parent.first_child().as_ref(),
        )
        .ok();
}

pub fn websocket_url() -> Res<Option<(String, String)>> {
    let win = match window() {
        Ok(w) => w,
        Err(_) => return err("Failed to read window Location."),
    };
    let loc = win.location();
    let host = match loc.host() {
        Ok(h) => h,
        Err(_) => return err("Failed to read window host."),
    };

    match (loc.pathname(), loc.protocol()) {
        (Ok(path), Ok(protocol)) => {
            let mut parts = path.split('/').collect::<Vec<&str>>();
            parts.retain(|p| !p.is_empty());
            match parts[..] {
                ["game", game_key] => Ok(Some((
                    format!(
                        "{}://{}/api/game/{}",
                        if protocol.contains('s') { "wss" } else { "ws" },
                        &host,
                        game_key,
                    ),
                    game_key.to_string(),
                ))),
                _ => Ok(None),
            }
        }
        _ => err("Failed to read window pathname."),
    }
}

fn get_window_dimensions() -> Res<(u32, u32)> {
    let win = window()?;

    match (win.inner_width(), win.inner_height()) {
        (Ok(w), Ok(h)) => match (w.as_f64(), h.as_f64()) {
            (Some(w), Some(h)) => Ok((w as u32, h as u32)),
            _ => err("Window dimensions non-numeric."),
        },
        _ => err("No Window dimensions."),
    }
}

fn create_file_upload() -> Res<HtmlInputElement> {
    let element = Element::try_new("input")?;

    element.set_attr("type", "file");
    element.set_attr("accept", "image/*");

    element
        .raw()
        .dyn_into::<HtmlInputElement>()
        .map_err(|_| "Failed to cast element to HtmlInputElement.".to_string())
}

pub fn request_animation_frame(f: &Closure<dyn FnMut()>) -> Res<()> {
    match window()?.request_animation_frame(f.as_ref().unchecked_ref()) {
        Ok(_) => Ok(()),
        Err(_) => err("Failed to get animation frame."),
    }
}

fn set_visible(id: &str, visible: bool) -> Res<()> {
    Element::by_id(id)
        .ok_or_else(|| format!("Element not found: {id}."))?
        .set_css("display", if visible { "" } else { "none" });
    Ok(())
}

fn set_checked(id: &str) -> Res<()> {
    if let Some(element) = get_document()?.get_element_by_id(id) {
        element
            .unchecked_ref::<HtmlInputElement>()
            .set_checked(true);
    }

    Ok(())
}

fn enum_to_id(tool: impl serde::Serialize, pref: &str) -> Res<String> {
    if let Ok(s) = serde_json::ser::to_string(&tool) {
        Ok(format!("{pref}{}", s.to_lowercase().replace('"', "")))
    } else {
        err("Serialisation error.")
    }
}

pub fn set_active_draw_tool(tool: impl serde::Serialize) -> Res<()> {
    const ID_PREFIX: &str = "draw_radio_";
    set_checked(&enum_to_id(tool, ID_PREFIX)?)
}

/// Random float in \[0, 1.0\] using JS Math.random()
pub fn rand() -> f32 {
    #[allow(unused_unsafe)]
    let num = unsafe { js_sys::Math::random() };
    num as f32
}

pub fn redirect_to(url: &str) {
    // Ensure we don't redirect multiple times.
    static REDIRECTED: AtomicBool = AtomicBool::new(false);
    if !REDIRECTED.load(std::sync::atomic::Ordering::Acquire) {
        if let Ok(window) = window() {
            window.location().set_href(url).ok();
            REDIRECTED.store(true, std::sync::atomic::Ordering::Release);
        }
    }
}

pub fn game_over_redirect() {
    const HREF: &str = "/game_over";
    redirect_to(HREF);
}

pub fn timestamp_ms() -> u64 {
    js_sys::Date::new_0().get_time() as u64
}

fn js_err(v: JsValue) -> String {
    if let Some(s) = v.as_string() {
        s
    } else {
        format!("{v:?}")
    }
}

pub struct SaveState {
    onload: Closure<dyn FnMut(JsValue)>,
    onerror: Closure<dyn FnMut(JsValue)>,
    promise: Promise,
}

impl SaveState {
    fn new<L, E>(onload: L, onerror: E, req: Promise) -> Self
    where
        L: FnMut(JsValue) + 'static,
        E: FnMut(JsValue) + 'static,
    {
        let onload = Closure::new(onload);
        let onerror = Closure::new(onerror);
        let promise = req.then(&onload).catch(&onerror);
        Self {
            onload,
            onerror,
            promise,
        }
    }
}

pub fn save_scene(scene_key: &str, raw: Vec<u8>) -> Res<SaveState> {
    #[derive(serde_derive::Serialize)]
    struct Req {
        encoded: String,
    }

    #[derive(serde_derive::Deserialize)]
    struct Resp {
        message: String,
        project_title: String,
        project_key: String,
        project_id: i64,
        scene: String,
        scene_key: String,
        success: bool,
        title: String,
    }

    const METHOD: &str = "PUT";
    const PATH: &str = "/api/scene/";

    let headers = Headers::new().map_err(js_err)?;
    headers
        .set("content-type", "application/json")
        .map_err(js_err)?;

    let body = serde_json::ser::to_string(&Req {
        encoded: base64::encode(raw),
    })
    .map_err(|e| e.to_string())?;

    if let Some(loading) = Element::by_id("canvas_loading_icon") {
        loading.show();
        loading.remove_class("loading-idle");
        loading.remove_class("loading-error");
        loading.add_class("loading-loading");
        loading.set_attr("title", "Saving scene");
    }

    let mut init = RequestInit::new();
    init.method(METHOD)
        .headers(&headers)
        .body(Some(&wasm_bindgen::JsValue::from_str(&body)));
    let req = Request::new_with_str_and_init(&format!("{PATH}{scene_key}"), &init)
        .map_err(|s| s.as_string().unwrap_or_else(|| format!("{s:?}")))?;
    let promise = window()?.fetch_with_request(&req);

    Ok(SaveState::new(
        |resp: JsValue| {
            if let Some(loading) = Element::by_id("canvas_loading_icon") {
                loading.remove_class("loading-loading");
                if resp.unchecked_into::<Response>().ok() {
                    loading.hide();
                } else {
                    loading.add_class("loading-error");
                    loading.set_attr("title", "Failed to save");
                }
            }
        },
        |_err| {
            if let Some(loading) = Element::by_id("canvas_loading_icon") {
                loading.remove_class("loading-loading");
                loading.add_class("loading-error");
                loading.set_attr("title", "Network error");
            }
        },
        promise,
    ))
}
