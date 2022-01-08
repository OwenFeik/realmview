use std::any::Any;
use std::rc::Rc;
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
    MouseEvent,
    ProgressEvent,
    UiEvent,
    Url,
    WebGlBuffer,
    WebGlProgram,
    WebGlShader,
    WebGlTexture,
    WebGlUniformLocation,
    WebGl2RenderingContext,
    Window
};

type Gl = WebGl2RenderingContext;

// 0 is the default and what is used here
const GL_TEXTURE_DETAIL_LEVEL: i32 = 0;

// Required to be 0 for textures
const GL_TEXTURE_BORDER_WIDTH: i32 = 0;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_bool(b: bool);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_int(i: i32);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_js_value(v: &JsValue);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_texture(v: &WebGlTexture);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_arr(a: &[f32]);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_float(f: f32);
}

#[derive(Debug)]
enum JsError {
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

struct Program {
    gl: Rc<Gl>,
    program: WebGlProgram,
    position_buffer: WebGlBuffer,
    position_location: u32,
    texcoord_buffer: WebGlBuffer,
    texcoord_location: u32,
    matrix_location: WebGlUniformLocation,
    texture_location: WebGlUniformLocation
}

// GLSL Shaders for the program

const VERTEX_SHADER: &str = "
attribute vec4 a_position;
attribute vec2 a_texcoord;

uniform mat4 u_matrix;

varying vec2 v_texcoord;

void main() {
    gl_Position = u_matrix * a_position;
    v_texcoord = a_texcoord;
}
";

const FRAGMENT_SHADER: &str = "
precision mediump float;

varying vec2 v_texcoord;

uniform sampler2D u_texture;

void main() {
    gl_FragColor = texture2D(u_texture, v_texcoord);
}
";

impl Program {
    fn new(gl: Rc<Gl>) -> Result<Program, JsError> {
        let program = match gl.create_program() {
            Some(p) => p,
            None => return Err(JsError::ResourceError("WebGL program creation failed."))
        };
    
        gl.attach_shader(&program, &create_shader(&gl, VERTEX_SHADER, Gl::VERTEX_SHADER)?);
        gl.attach_shader(&program, &create_shader(&gl, FRAGMENT_SHADER, Gl::FRAGMENT_SHADER)?);
    
        gl.link_program(&program);
    
        if gl.get_program_parameter(&program, Gl::LINK_STATUS).is_falsy() {
            return Err(JsError::ResourceError("WebGL program linking failed."));
        }
    
        let position_location = gl.get_attrib_location(&program, "a_position") as u32;
        const POSITIONS: [f32; 12] = [
            0.0, 0.0,
            0.0, 1.0,
            1.0, 0.0,
            1.0, 0.0,
            0.0, 1.0,
            1.0, 1.0
        ];
        let position_buffer = create_buffer(&gl, &POSITIONS)?;
    
        let texcoord_location = gl.get_attrib_location(&program, "a_texcoord") as u32;
        const TEXCOORDS: [f32; 12] = [
            0.0, 0.0,
            0.0, 1.0,
            1.0, 0.0,
            1.0, 0.0,
            0.0, 1.0,
            1.0, 1.0
        ];
        let texcoord_buffer = create_buffer(&gl, &TEXCOORDS)?;

        let matrix_location = match gl.get_uniform_location(&program, "u_matrix") {
            Some(p) => p,
            None => return Err(JsError::ResourceError("Couldn't find shader matrix location."))
        };
        let texture_location = match gl.get_uniform_location(&program, "u_texture") {
            Some(l) => l,
            None => return Err(JsError::ResourceError("Couldn't find texture matrix location."))
        };

        Ok(
            Program {
                gl,
                program,
                position_buffer,
                position_location,
                texcoord_buffer,
                texcoord_location,
                matrix_location,
                texture_location
            }
        )
    }

    fn new_rc(gl: Rc<Gl>) -> Result<Rc<Program>, JsError> {
        Ok(Rc::new(Program::new(gl)?))
    }

    fn draw_image(&self, texture: Texture, x: f32, y: f32, vp_w: f32, vp_h: f32) {
        let gl = &self.gl;

        gl.viewport(0, 0, vp_w as i32, vp_h as i32);
        gl.clear_color(0.0, 0.0, 0.0, 1.0);
        gl.clear(Gl::COLOR_BUFFER_BIT);

        gl.active_texture(Gl::TEXTURE0);

        gl.bind_texture(Gl::TEXTURE_2D, Some(&texture.texture));

        gl.use_program(Some(&self.program));

        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.position_buffer));
        gl.enable_vertex_attrib_array(self.position_location);
        gl.vertex_attrib_pointer_with_i32(self.position_location, 2, Gl::FLOAT, false, 0, 0);
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.texcoord_buffer));
        gl.enable_vertex_attrib_array(self.texcoord_location);
        gl.vertex_attrib_pointer_with_i32(self.texcoord_location, 2, Gl::FLOAT, false, 0, 0);
    
        let mut matrix = m4_orthographic(0.0, vp_w, vp_h, 0.0, -1.0, 1.0);
        m4_translate(&mut matrix, x, y, 0.0);
        m4_scale(&mut matrix, texture.width as f32, texture.height as f32, 1.0);

        gl.uniform_matrix4fv_with_f32_array(Some(&self.matrix_location), false, &matrix);
        gl.uniform1i(Some(&self.texture_location), 0);
        gl.draw_arrays(Gl::TRIANGLES, 0, 6);
    }
}

struct Canvas {
    element: Rc<HtmlCanvasElement>,
    gl: Rc<Gl>,
    program: Rc<Program>
}

impl Canvas {
    fn new(element: HtmlCanvasElement) -> Result<Canvas, JsError> {
        let gl = Rc::new(create_context(&element)?);

        Ok(Canvas { element: Rc::new(element), gl: gl.clone(), program: Program::new_rc(gl)? })
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
        self.configure_upload()?;
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
        let closure = Closure::wrap(Box::new(
            move |_event: UiEvent| {
                Canvas::fill_window(&canvas).ok();
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

    fn configure_upload(&self) -> Result<(), JsError> {
        let input = Rc::new(create_file_upload()?);
        let result = {
            let c_input = input.clone();
            let gl = self.gl.clone();
            let program = self.program.clone();
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
                    let pr_ref = program.clone();
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

                            let pr_ref = pr_ref.clone();
                            Texture::from_url(
                                &gl_ref,
                                &url[..],
                                Box::new(move |res| {
                                    match res {
                                        Ok(texture) => pr_ref.draw_image(texture, 0.0, 0.0, 1400 as f32, 957 as f32),
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
                move |_event: MouseEvent| { input.click(); }
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


struct Texture {
    width: u32,
    height: u32,
    texture: Rc<WebGlTexture>
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

    fn load_html_image_gl_texture(
        gl: &Gl,
        image: &HtmlImageElement,
        texture: &WebGlTexture
    ) -> Result<(), JsError> {
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

fn create_context(element: &HtmlCanvasElement) -> Result<Gl, JsError> {
    match element.get_context("webgl2") {
        Ok(Some(c)) => match c.dyn_into::<Gl>() {
            Ok(c) => Ok(c),
            Err(_) => return Err(JsError::TypeError("Failed to cast to WebGL context."))
        },
        _ => return Err(JsError::ResourceError("Failed to get rendering context."))
    }
}

fn create_shader(gl: &Gl, src: &str, stype: u32) -> Result<WebGlShader, JsError> {
    let shader = match gl.create_shader(stype) {
        Some(s) => s,
        None => return Err(JsError::ResourceError("Failed to create shader."))
    };

    gl.shader_source(&shader, src);
    gl.compile_shader(&shader);

    if gl.get_shader_parameter(&shader, Gl::COMPILE_STATUS).is_falsy() {
        return match gl.get_shader_info_log(&shader) {
            Some(_) => Err(JsError::ResourceError("Shader compilation failed.")),
            None => Err(JsError::ResourceError("Shader compilation failed, no error message."))
        }
    }

    Ok(shader)
}

fn create_buffer(gl: &Gl, data: &[f32]) -> Result<WebGlBuffer, JsError> {
    let buffer = match gl.create_buffer() {
        Some(b) => b,
        None => return Err(JsError::ResourceError("Failed to create WebGL buffer."))
    };

    gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&buffer));

    let arr = unsafe { js_sys::Float32Array::view(data) };
    gl.buffer_data_with_opt_array_buffer(
        Gl::ARRAY_BUFFER,
        Some(&arr.buffer()),
        Gl::STATIC_DRAW
    );

    Ok(buffer)
}

// see https://webglfundamentals.org/webgl/resources/m4.js
fn m4_orthographic(l: f32, r: f32, b: f32, t: f32, n: f32, f: f32) -> [f32; 16] {
    [
        2.0 / (r - l)    , 0.0              , 0.0              , 0.0,
        0.0              , 2.0 / (t - b)    , 0.0              , 0.0,
        0.0              , 0.0              , 2.0 / (n - f)    , 0.0,
        (l + r) / (l - r), (b + t) / (b - t), (n + f) / (n - f), 1.0
    ]
}

// Translates matrix m by tx units in the x direction and likewise for ty and tz.
// NB: in place
fn m4_translate(m: &mut [f32; 16], tx: f32, ty: f32, tz: f32) {
    m[12] = m[0] * tx + m[4] * ty + m[8] * tz + m[12];
    m[13] = m[1] * tx + m[5] * ty + m[9] * tz + m[13];
    m[14] = m[2] * tx + m[6] * ty + m[10] * tz + m[14];
    m[15] = m[3] * tx + m[7] * ty + m[11] * tz + m[15];
}

// NB: in place
fn m4_scale(m: &mut [f32; 16], sx: f32, sy: f32, sz: f32) {
    m[0] = m[0] * sx;
    m[1] = m[1] * sx;
    m[2] = m[2] * sx;
    m[3] = m[3] * sx;
    m[4] = m[4] * sy;
    m[5] = m[5] * sy;
    m[6] = m[6] * sy;
    m[7] = m[7] * sy;
    m[8] = m[8] * sz;
    m[9] = m[9] * sz;
    m[10] = m[10] * sz;
    m[11] = m[11] * sz;
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
        .or(Err(JsError::TypeError("Failed to cast element to HtmlInputElement")))
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    Canvas::new_element().unwrap();

    Ok(())
}
