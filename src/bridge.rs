use std::rc::Rc;
use js_sys::Array;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    Blob,
    Document,
    FileReader,
    HtmlCanvasElement,
    HtmlElement,
    HtmlImageElement,
    HtmlInputElement,
    InputEvent,
    ProgressEvent,
    UiEvent,
    Url,
    WebGlTexture,
    WebGl2RenderingContext,
    Window
};

use crate::programs::GridRenderer;
use crate::programs::TextureRenderer;
use crate::scene::{Rect, Sprite};

#[wasm_bindgen]
extern "C" {
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

    fn set_up_uploads(closure: &Closure<dyn FnMut(&JsValue)>);
}


pub type Gl = WebGl2RenderingContext;


// 0 is the default and what is used here
const GL_TEXTURE_DETAIL_LEVEL: i32 = 0;


// Required to be 0 for textures
const GL_TEXTURE_BORDER_WIDTH: i32 = 0;


#[derive(Debug)]
pub enum JsError {
    ResourceError(&'static str),
    TypeError(&'static str)
}


struct Element {
    element: HtmlElement
}


impl Element {
    fn new(name: &str) -> Result<Element, JsError> {
        match create_element(name)?.dyn_into::<HtmlElement>() {
            Ok(e) => Ok(Element { element: e }),
            Err(_) => Err(
                JsError::TypeError("Couldn't cast to HtmlElement.")
            )
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
            .or(Err(JsError::ResourceError("Failed to set element attribute.")))
    }
}


struct Canvas {
    element: Rc<HtmlCanvasElement>,
    gl: Rc<Gl>,
    
    // Array where MouseEvents are stored to be handled by the core loop.
    events: Rc<Array>
}


impl Canvas {
    fn new(element: HtmlCanvasElement) -> Result<Canvas, JsError> {
        let gl = Rc::new(create_context(&element)?);

        Ok(Canvas { element: Rc::new(element), gl, events: Rc::new(Array::new()) })
    }

    /// Create a new canvas element and set it up to fill the screen.
    fn new_element() -> Result<Canvas, JsError> {
        let element = create_appended("canvas")?;
        let canvas = match element.dyn_into::<HtmlCanvasElement>() {
            Ok(c) => Canvas::new(c)?,
            Err(_) => return Err(JsError::TypeError("Couldn't cast Element to HtmlCanvas."))
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
        let closure = Closure::wrap(Box::new(
            move |_event: UiEvent| {
                Canvas::fill_window(&canvas).ok();
                gl.viewport(0, 0, canvas.width() as i32, canvas.height() as i32);
            }
        ) as Box<dyn FnMut(_)>);    

        let result = get_window()?.add_event_listener_with_callback(
            "resize",
            closure.as_ref().unchecked_ref()
        );
        closure.forget();
        result.or(
            Err(JsError::ResourceError("Failed to add resize listener."))
        )
    }

    fn configure_events(&self) -> Result<(), JsError> {
        for event_name in vec!["mousedown", "mouseup", "mousemove"].iter() {
            let events = self.events.clone();
            let listener = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
                events.push(&event);
            }) as Box<dyn FnMut(web_sys::MouseEvent)>);

            match self.element.add_event_listener_with_callback(event_name, &listener.as_ref().unchecked_ref()) {
                Ok(_) => (),
                Err(_) => return Err(JsError::ResourceError("Failed to add mouse event listener to canvas."))
            };

            listener.forget();
        }
        
        Ok(())
    }

    fn configure_upload(&self) -> Result<(), JsError> {
        let input = Rc::new(create_file_upload()?);
        let result = {
            let c_input = input.clone();
            let gl = self.gl.clone();
            let closure = Closure::wrap(Box::new(
                move |_event: InputEvent| {
                    let file = match c_input.files() {
                        Some(fs) => match fs.get(0) {
                            Some(f) => f,
                            None => return
                        },
                        None => return
                    };

                    let file_reader = match FileReader::new() {
                        Ok(fr) => Rc::new(fr),
                        Err(_) => return
                    };

                    // File load handling
                    let fr_ref = file_reader.clone();
                    let gl_ref = gl.clone();
                    let closure = Closure::wrap(Box::new(
                        move |_event: ProgressEvent| {
                            let file = match fr_ref.result() {
                                Ok(f) => f,
                                Err(_) => return
                            };

                            let array = js_sys::Array::new(); 
                            array.push(&file);                           
                            
                            let result =
                                Blob::new_with_buffer_source_sequence(&array);

                            let blob = match result {
                                Ok(b) => b, Err(_) => return
                            };

                            let url =
                            match Url::create_object_url_with_blob(&blob) {
                                Ok(s) => s,
                                Err(_) => return
                            };

                            Texture::from_url(
                                &gl_ref,
                                &url[..],
                                Box::new(move |res| {
                                    match res {
                                        Ok(_t) => return,
                                        Err(_) => return
                                    }
                                })
                            ).ok();
                        }
                    ) as Box<dyn FnMut(_)>);
                    
                    if let Err(_) = file_reader.add_event_listener_with_callback(
                        "loadend",
                        closure.as_ref().unchecked_ref()
                    ) {
                        return;
                    }
                    closure.forget();

                    if let Err(_) = file_reader.read_as_array_buffer(&file) {
                        return;
                    }

                    ()
                }
            ) as Box<dyn FnMut(_)>);
            let result = input.add_event_listener_with_callback(
                "input",
                closure.as_ref().unchecked_ref()
            );
            closure.forget();
            result
        };

        match result {
            Ok(()) => (),
            Err(_) => return Err(
                JsError::ResourceError("Failed to add event listener.")
            )
        };

        {
            let input = input.clone();
            let closure = Closure::wrap(Box::new(
                move |_event: web_sys::MouseEvent| { input.click(); }
            ) as Box<dyn FnMut(_)>);
            let result = self.element.add_event_listener_with_callback(
                "click",
                closure.as_ref().unchecked_ref()
            );
            closure.forget();
            result
        }.or(Err(JsError::ResourceError("Failed to add click listener.")))
    }
}


pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub texture: Rc<WebGlTexture>
}


impl Texture {
    fn new(gl: &Gl) -> Result<Texture, JsError> {
        Ok(
            Texture {
                width: 0,
                height: 0,
                texture: Rc::new(Texture::create_gl_texture(gl)?)
            }
        )
    }

    fn gen_mipmap(&self, gl: &Gl) {
        gl.bind_texture(Gl::TEXTURE_2D, Some(&self.texture));
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MIN_FILTER, Gl::LINEAR as i32);
    }

    fn create_gl_texture(
        gl: &Gl
    ) -> Result<WebGlTexture, JsError> {
        match gl.create_texture() {
            Some(t) => Ok(t),
            None => return Err(
                JsError::ResourceError("Unable to create texture.")
            )
        }
    }

    fn load_u8_array(
        &mut self,
        gl: &Gl,
        width: u32,
        height: u32,
        data: &[u8]
    ) -> Result<(), JsError> {
        gl.bind_texture(Gl::TEXTURE_2D, Some(&self.texture));

        if let Err(_) = gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            Gl::TEXTURE_2D,
            GL_TEXTURE_DETAIL_LEVEL,
            Gl::RGBA as i32,
            width as i32,
            height as i32,
            GL_TEXTURE_BORDER_WIDTH,
            Gl::RGBA,
            Gl::UNSIGNED_BYTE, // u8
            Some(data)
        ) {
            return Err(JsError::ResourceError("Unable to load array."));
        }

        self.gen_mipmap(gl);

        self.width = width;
        self.height = height;

        Ok(())
    }

    fn from_u8_array(
        gl: &Gl,
        width: u32,
        height: u32,
        data: &[u8]
    ) -> Result<Texture, JsError> {
        let mut texture = Texture::new(gl)?;
        texture.load_u8_array(gl, width, height, data)?;
        Ok(texture)
    }

    fn from_html_image(gl: &Gl, image: &HtmlImageElement) -> Result<Texture, JsError> {
        let mut texture = Texture::new(gl)?;
        texture.load_html_image(gl, image)?;

        Ok(texture)
    }

    fn load_html_image(&mut self, gl: &Gl, image: &HtmlImageElement) -> Result<(), JsError> {
        Texture::load_html_image_gl_texture(gl, image, &self.texture)?;
        self.width = image.natural_width();
        self.height = image.natural_height();
        self.gen_mipmap(gl);

        Ok(())
    }

    fn load_html_image_gl_texture(gl: &Gl, image: &HtmlImageElement, texture: &WebGlTexture) -> Result<(), JsError> {
        gl.bind_texture(Gl::TEXTURE_2D, Some(texture));

        if let Err(_) = gl.tex_image_2d_with_u32_and_u32_and_html_image_element(
            Gl::TEXTURE_2D,
            GL_TEXTURE_DETAIL_LEVEL,
            Gl::RGBA as i32,
            Gl::RGBA,
            Gl::UNSIGNED_BYTE,
            image
        ) {
            return Err(JsError::ResourceError("Failed to create WebGL image."));
        }
    
        Ok(())
    }

    fn from_url(
        gl: &Gl,
        url: &str,
        callback: Box<dyn Fn(Result<Texture, JsError>)>
    ) -> Result<(), JsError> {
        // Create HTML image to load image from url
        let image = match HtmlImageElement::new() {
            Ok(i) => Rc::new(i),
            Err(_) => return Err(
                JsError::ResourceError("Unable to create image element.")
            )
        };
        image.set_cross_origin(Some("")); // ?
    
        // Set callback to update texture once image is loaded
        {
            let gl = Rc::new(gl.clone());
            let image_ref = image.clone();
            let closure = Closure::wrap(Box::new(move || {
                callback(Texture::from_html_image(&gl, &image_ref));
            }) as Box<dyn FnMut()>);
            image.set_onload(Some(closure.as_ref().unchecked_ref()));
            closure.forget();
        }
    
        // Load image
        image.set_src(url);
    
        Ok(())
    }
}


pub enum EventType {
    MouseDown,
    MouseUp,
    MouseMove
}


pub struct MouseEvent {
    pub x: i32,
    pub y: i32,
    pub event_type: EventType
}


impl MouseEvent {
    fn from_web_sys(event: &web_sys::MouseEvent) -> Option<MouseEvent> {
        let event_type = match event.type_().as_str() {
            "mousedown" => EventType::MouseDown,
            "mouseup" => EventType::MouseUp,
            "mousemove" => EventType::MouseMove,
            _ => return None
        };

        Some(MouseEvent { x: event.x(), y: event.y(), event_type })
    }
}


pub struct Context {
    // WebGL context. Wrapped in Rc because various structs and closures want for references to it.
    gl: Rc<Gl>,

    // Holds information about the HTML canvas associated with the WebGL context.
    canvas: Canvas,

    // Rendering program, used to draw sprites.
    texture_renderer: TextureRenderer,

    // To render map grid
    grid_renderer: GridRenderer,
    
    // A JS Array which the front end pushes uploaded images to. The Context then loads any images waiting in the queue
    // before rendering each frame. Wrapped in Rc such that it can be accessed from a closure passed to JS.
    texture_queue: Rc<Array>,
}


impl Context {
    pub fn new() -> Result<Context, JsError> {
        let canvas = Canvas::new_element()?;
        let texture_renderer = TextureRenderer::new(canvas.gl.clone())?;
        let grid_renderer = GridRenderer::new(canvas.gl.clone())?;
        let ctx = Context{
            gl: canvas.gl.clone(),
            canvas,
            texture_renderer,
            grid_renderer,
            texture_queue: Rc::new(Array::new()),
        };
        ctx.configure_upload();

        Ok(ctx)
    }

    fn configure_upload(&self) {
        let texture_queue = self.texture_queue.clone();
        let handler = Closure::wrap(Box::new(move |image: &JsValue| {
            texture_queue.push(&image);
        }) as Box<dyn FnMut(&JsValue)>);
        set_up_uploads(&handler); // rust-analyzer dislikes but this compiles and functions fine.
        handler.forget();
    }

    pub fn viewport(&self) -> Rect {
        Rect {
            x: 0,
            y: 0,
            w: self.canvas.element.width() as i32,
            h: self.canvas.element.height() as i32
        }
    }

    pub fn events(&self) -> Option<Vec<MouseEvent>> {
        if self.canvas.events.length() == 0 {
            return None;
        }

        let mut events = Vec::new();
        while self.canvas.events.length() > 0 {
            let event = self.canvas.events.pop();
            let event = event.unchecked_ref::<web_sys::MouseEvent>();
            match MouseEvent::from_web_sys(event) {
                Some(e) => events.push(e),
                None => ()
            };
        }

        match events.len() {
            0 => None,
            _ => Some(events)
        }
    }

    pub fn load_queue(&self) -> Option<Vec<Sprite>> {
        if self.texture_queue.length() == 0 {
            return None;
        }

        let mut sprites = Vec::new();
        while self.texture_queue.length() > 0 {
            let img = self.texture_queue.pop();
            
            // Cast the img to a HTMLImageElement; this array will only contain such elements, so this cast is safe.
            let img = img.unchecked_ref::<HtmlImageElement>();
            match Texture::from_html_image(&self.gl, img) {
                Ok(t) => sprites.push(Sprite::new(t)),
                Err(_) => ()
            };
        }

        Some(sprites)
    }

    pub fn render(&mut self, sprites: &Vec<Sprite>, grid_size: i32) {
        let vp = self.viewport();

        self.gl.viewport(0, 0, vp.w, vp.h);
        self.gl.clear(Gl::COLOR_BUFFER_BIT);
        
        for sprite in sprites.iter() {
            self.texture_renderer.draw_texture(&vp, sprite.texture(), &sprite.absolute_rect(grid_size));
        }

        self.grid_renderer.render_grid(vp, grid_size);
    }
}


fn create_context(element: &HtmlCanvasElement) -> Result<Gl, JsError> {
    let gl = match element.get_context("webgl2") {
        Ok(Some(c)) => match c.dyn_into::<Gl>() {
            Ok(c) => c,
            Err(_) => return Err(JsError::TypeError("Failed to cast to WebGL context."))
        },
        _ => return Err(JsError::ResourceError("Failed to get rendering context."))
    };

    // Enable transparency
    gl.enable(Gl::BLEND);
    gl.blend_func(Gl::SRC_ALPHA, Gl::ONE_MINUS_SRC_ALPHA);

    Ok(gl)
}


fn get_window() -> Result<Window, JsError> {
    match web_sys::window() {
        Some(w) => Ok(w),
        None => Err(JsError::ResourceError("No Window."))
    }
}


fn get_document() -> Result<Document, JsError> {
    match get_window()?.document() {
        Some(d) => Ok(d),
        None => Err(JsError::ResourceError("No Document."))
    }
}


fn get_body() -> Result<HtmlElement, JsError> {
    match get_document()?.body() {
        Some(b) => Ok(b),
        None => Err(JsError::ResourceError("No Body."))
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
        Err(_) => Err(JsError::ResourceError("Failed to append element."))
    }
}


fn get_window_dimensions() -> Result<(u32, u32), JsError> {
    let window = get_window()?;

    match (window.inner_width(), window.inner_height()) {
        (Ok(w), Ok(h)) => match (w.as_f64(), h.as_f64()) {
            (Some(w), Some(h)) => Ok((w as u32, h as u32)),
            _ => return Err(JsError::TypeError("Window dimensions non-numeric."))
        },
        _ => return Err(JsError::ResourceError("No Window dimensions."))
    }
}


fn create_file_upload() -> Result<HtmlInputElement, JsError> {
    let element = Element::new("input")?;

    element.set_attr("type", "file")?;
    element.set_attr("accept", "image/*")?;

    element
        .element
        .dyn_into::<HtmlInputElement>()
        .or(Err(JsError::TypeError("Failed to cast element to HtmlInputElement.")))
}


pub fn request_animation_frame(f: &Closure<dyn FnMut()>) -> Result<(), JsError> {
    match get_window()?.request_animation_frame(f.as_ref().unchecked_ref()) {
        Ok(_) => Ok(()),
        Err(_) => Err(JsError::ResourceError("Failed to get animation frame."))
    }
}
