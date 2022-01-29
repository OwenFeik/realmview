use std::collections::HashMap;
use std::rc::Rc;

use js_sys::Float32Array;
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{
    HtmlImageElement, WebGlBuffer, WebGlProgram, WebGlShader, WebGlTexture, WebGlUniformLocation,
};

use crate::bridge::{Gl, JsError};
use crate::scene::Rect;

type Colour = [f32; 4];

// 0 is the default and what is used here
const GL_TEXTURE_DETAIL_LEVEL: i32 = 0;

// Required to be 0 for textures
const GL_TEXTURE_BORDER_WIDTH: i32 = 0;

struct Texture {
    pub width: u32,
    pub height: u32,
    pub texture: WebGlTexture,
}

impl Texture {
    fn new(gl: &Gl) -> Result<Texture, JsError> {
        Ok(Texture {
            width: 0,
            height: 0,
            texture: Texture::create_gl_texture(gl)?,
        })
    }

    fn gen_mipmap(&self, gl: &Gl) {
        gl.bind_texture(Gl::TEXTURE_2D, Some(&self.texture));
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MIN_FILTER, Gl::LINEAR as i32);
    }

    fn create_gl_texture(gl: &Gl) -> Result<WebGlTexture, JsError> {
        match gl.create_texture() {
            Some(t) => Ok(t),
            None => Err(JsError::ResourceError("Unable to create texture.")),
        }
    }

    fn load_u8_array(
        &mut self,
        gl: &Gl,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Result<(), JsError> {
        gl.bind_texture(Gl::TEXTURE_2D, Some(&self.texture));

        if gl
            .tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
                Gl::TEXTURE_2D,
                GL_TEXTURE_DETAIL_LEVEL,
                Gl::RGBA as i32,
                width as i32,
                height as i32,
                GL_TEXTURE_BORDER_WIDTH,
                Gl::RGBA,
                Gl::UNSIGNED_BYTE, // u8
                Some(data),
            )
            .is_err()
        {
            return Err(JsError::ResourceError("Unable to load array."));
        }

        self.gen_mipmap(gl);

        self.width = width;
        self.height = height;

        Ok(())
    }

    fn from_u8_array(gl: &Gl, width: u32, height: u32, data: &[u8]) -> Result<Texture, JsError> {
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
        texture: &WebGlTexture,
    ) -> Result<(), JsError> {
        gl.bind_texture(Gl::TEXTURE_2D, Some(texture));

        if gl
            .tex_image_2d_with_u32_and_u32_and_html_image_element(
                Gl::TEXTURE_2D,
                GL_TEXTURE_DETAIL_LEVEL,
                Gl::RGBA as i32,
                Gl::RGBA,
                Gl::UNSIGNED_BYTE,
                image,
            )
            .is_err()
        {
            return Err(JsError::ResourceError("Failed to create WebGL image."));
        }

        Ok(())
    }

    fn from_url(
        gl: &Gl,
        url: &str,
        callback: Box<dyn Fn(Result<Texture, JsError>)>,
    ) -> Result<(), JsError> {
        // Create HTML image to load image from url
        let image = match HtmlImageElement::new() {
            Ok(i) => Rc::new(i),
            Err(_) => return Err(JsError::ResourceError("Unable to create image element.")),
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

struct TextureManager {
    gl: Rc<Gl>,
    textures: HashMap<u32, Texture>,
}

impl TextureManager {
    fn new(gl: Rc<Gl>) -> Result<TextureManager, JsError> {
        let missing_texture = Texture::from_u8_array(&gl, 1, 1, &[0, 0, 255, 255])?;
        let mut tm = TextureManager {
            gl,
            textures: HashMap::new(),
        };
        tm.add_texture(0, missing_texture);
        Ok(tm)
    }

    fn load_image(&mut self, image: &HtmlImageElement) -> u32 {
        let id = match image.get_attribute("data-id") {
            Some(s) => s.parse::<u32>().unwrap_or(0),
            None => 0,
        };

        if id != 0 {
            match Texture::from_html_image(&self.gl, image) {
                Ok(t) => self.textures.insert(id, t),
                Err(_) => return 0,
            };
        }

        id
    }

    // NB will overwrite existing texture of this id
    fn add_texture(&mut self, id: u32, texture: Texture) {
        self.textures.insert(id, texture);
    }

    fn get_texture(&self, id: u32) -> &WebGlTexture {
        if let Some(tex) = self.textures.get(&id) {
            &tex.texture
        } else {
            // This unwrap is safe because we always add a missing texture
            // texture as id 0 in the constructor.
            &self.textures.get(&0).unwrap().texture
        }
    }
}

struct TextureRenderer {
    gl: Rc<Gl>,
    program: WebGlProgram,
    position_buffer: WebGlBuffer,
    position_location: u32,
    texcoord_buffer: WebGlBuffer,
    texcoord_location: u32,
    matrix_location: WebGlUniformLocation,
    texture_location: WebGlUniformLocation,
}

// GLSL Shaders for the TextureRenderer program

const IMAGE_VERTEX_SHADER: &str = "
attribute vec4 a_position;
attribute vec2 a_texcoord;

uniform mat4 u_matrix;

varying vec2 v_texcoord;

void main() {
    gl_Position = u_matrix * a_position;
    v_texcoord = a_texcoord;
}
";

const IMAGE_FRAGMENT_SHADER: &str = "
precision mediump float;

varying vec2 v_texcoord;

uniform sampler2D u_texture;

void main() {
    gl_FragColor = texture2D(u_texture, v_texcoord);
}
";

impl TextureRenderer {
    pub fn new(gl: Rc<Gl>) -> Result<TextureRenderer, JsError> {
        let program = create_program(&gl, IMAGE_VERTEX_SHADER, IMAGE_FRAGMENT_SHADER)?;

        let position_location = gl.get_attrib_location(&program, "a_position") as u32;
        let texcoord_location = gl.get_attrib_location(&program, "a_texcoord") as u32;

        // Just a square. A 4x4 matrix is used to transform this to the
        // appropriate dimensions when rendering textures.
        let coords = Float32Array::new_with_length(12);
        coords.copy_from(&[0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
        let position_buffer = create_buffer(&gl, Some(&coords))?;
        let texcoord_buffer = create_buffer(&gl, Some(&coords))?;

        let matrix_location = match gl.get_uniform_location(&program, "u_matrix") {
            Some(p) => p,
            None => {
                return Err(JsError::ResourceError(
                    "Couldn't find shader matrix location.",
                ))
            }
        };
        let texture_location = match gl.get_uniform_location(&program, "u_texture") {
            Some(l) => l,
            None => {
                return Err(JsError::ResourceError(
                    "Couldn't find texture matrix location.",
                ))
            }
        };

        Ok(TextureRenderer {
            gl,
            program,
            position_buffer,
            position_location,
            texcoord_buffer,
            texcoord_location,
            matrix_location,
            texture_location,
        })
    }

    pub fn draw_texture(&self, viewport: Rect, texture: &WebGlTexture, position: Rect) {
        let gl = &self.gl;

        gl.bind_texture(Gl::TEXTURE_2D, Some(texture));
        gl.use_program(Some(&self.program));
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.position_buffer));
        gl.enable_vertex_attrib_array(self.position_location);
        gl.vertex_attrib_pointer_with_i32(self.position_location, 2, Gl::FLOAT, false, 0, 0);
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.texcoord_buffer));
        gl.enable_vertex_attrib_array(self.texcoord_location);
        gl.vertex_attrib_pointer_with_i32(self.texcoord_location, 2, Gl::FLOAT, false, 0, 0);

        let mut matrix = m4_orthographic(0.0, viewport.w as f32, viewport.h as f32, 0.0, -1.0, 1.0);
        m4_translate(
            &mut matrix,
            (position.x - viewport.x) as f32,
            (position.y - viewport.y) as f32,
            0.0,
        );
        m4_scale(&mut matrix, position.w as f32, position.h as f32, 1.0);

        gl.uniform_matrix4fv_with_f32_array(Some(&self.matrix_location), false, &matrix);
        gl.uniform1i(Some(&self.texture_location), 0);
        gl.draw_arrays(Gl::TRIANGLES, 0, 6);
    }
}

pub struct LineRenderer {
    gl: Rc<Gl>,
    program: WebGlProgram,
    position_location: u32,
    position_buffer: WebGlBuffer,
    colour_location: WebGlUniformLocation,
    point_count: i32,
}

const LINE_VERTEX_SHADER: &str = "
attribute vec4 a_position;

void main() {
    gl_Position = a_position;
}
";

const LINE_FRAGMENT_SHADER: &str = "
precision mediump float;

uniform vec4 u_color;

void main() {
    gl_FragColor = u_color;
}
";

impl LineRenderer {
    fn new(gl: Rc<Gl>) -> Result<LineRenderer, JsError> {
        let program = create_program(&gl, LINE_VERTEX_SHADER, LINE_FRAGMENT_SHADER)?;
        let position_location = gl.get_attrib_location(&program, "a_position") as u32;
        let position_buffer = create_buffer(&gl, None)?;
        let colour_location = match gl.get_uniform_location(&program, "u_color") {
            Some(l) => l,
            None => {
                return Err(JsError::ResourceError(
                    "Failed to get location for colour vector.",
                ))
            }
        };

        Ok(LineRenderer {
            gl,
            program,
            position_location,
            position_buffer,
            colour_location,
            point_count: 0,
        })
    }

    fn scale_and_load_points(&mut self, points: &mut Vec<f32>, vp_w: f32, vp_h: f32) {
        for i in 0..points.len() {
            // Point vectors are of form [x1, y1, x2, y2 ... xn, yn] so even indices are xs.
            if i % 2 == 0 {
                points[i] = (2.0 * points[i] - vp_w) / vp_w;
            } else {
                points[i] = -(2.0 * points[i] - vp_h) / vp_h;
            }
        }
        self.load_points(points);
    }

    fn load_points(&mut self, points: &[f32]) {
        let positions = Float32Array::from(points);

        self.gl
            .bind_buffer(Gl::ARRAY_BUFFER, Some(&self.position_buffer));
        self.gl.buffer_data_with_opt_array_buffer(
            Gl::ARRAY_BUFFER,
            Some(&positions.buffer()),
            Gl::STATIC_DRAW,
        );
        self.point_count = (points.len() / 2) as i32;
    }

    fn prepare_render(&self, colour: Option<Colour>) {
        let gl = &self.gl;

        gl.use_program(Some(&self.program));
        gl.enable_vertex_attrib_array(self.position_location);
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.position_buffer));
        gl.vertex_attrib_pointer_with_i32(self.position_location, 2, Gl::FLOAT, false, 0, 0);
        gl.uniform4fv_with_f32_array(
            Some(&self.colour_location),
            &colour.unwrap_or([0.5, 0.5, 0.5, 0.75]),
        );
    }

    fn render_lines(&self, colour: Option<Colour>) {
        self.prepare_render(colour);
        self.gl.draw_arrays(Gl::LINES, 0, self.point_count);
    }

    fn render_line_loop(&self, colour: Option<Colour>) {
        self.prepare_render(colour);
        self.gl.draw_arrays(Gl::LINE_LOOP, 0, self.point_count);
    }

    fn render_solid(&self, colour: Option<Colour>) {
        self.prepare_render(colour);
        self.gl.draw_arrays(Gl::TRIANGLES, 0, self.point_count);
    }
}

pub struct GridRenderer {
    line_renderer: LineRenderer,
    current_grid_rect: Option<Rect>,
    current_grid_size: Option<f32>,
    current_line_count: Option<i32>,
}

impl GridRenderer {
    pub fn new(gl: Rc<Gl>) -> Result<GridRenderer, JsError> {
        Ok(GridRenderer {
            line_renderer: LineRenderer::new(gl)?,
            current_grid_rect: None,
            current_grid_size: None,
            current_line_count: None,
        })
    }

    pub fn create_grid(&mut self, vp: Rect, grid_size: f32) {
        let mut verticals = Vec::new();
        let mut horizontals = Vec::new();

        let d = grid_size;
        let dx = vp.x % grid_size;
        let dy = vp.y % grid_size;

        let w = vp.w;
        let h = vp.h;

        let mut finished = false;
        let mut i = 0.0;
        while !finished {
            finished = true;

            let mut x = d * i - dx;
            if x <= w {
                x = (2.0 * x - w) / w; // Map to [-1, 1]

                verticals.push(x);
                verticals.push(-1.0);
                verticals.push(x);
                verticals.push(1.0);
                finished = false;
            }

            let mut y = d * i - dy;
            if y <= h {
                // I negate the expression here but not for the x because the OpenGL coordinate system naturally matches
                // the browser coordinate system in the x direction, but opposes it in the y direction. By negating the
                // two coordinate systems are aligned, which makes things a little easier to work with.
                y = -(2.0 * y - h) / h;

                horizontals.push(-1.0);
                horizontals.push(y);
                horizontals.push(1.0);
                horizontals.push(y);
                finished = false;
            }

            i += 1.0;
        }

        verticals.append(&mut horizontals);
        self.line_renderer.load_points(&verticals);
        self.current_grid_rect = Some(vp);
        self.current_grid_size = Some(grid_size);
        self.current_line_count = Some(verticals.len() as i32 / 2);
    }

    pub fn render_grid(&mut self, vp: Rect, grid_size: f32) {
        if let Some(rect) = self.current_grid_rect {
            if rect != vp || self.current_grid_size != Some(grid_size) {
                self.create_grid(vp, grid_size);
            }
        } else {
            self.create_grid(vp, grid_size);
        }

        self.line_renderer.render_lines(None);
    }
}

pub struct Renderer {
    // Loads and stores references to textures
    texture_library: TextureManager,

    // Rendering program, used to draw sprites.
    texture_renderer: TextureRenderer,

    // To render outlines &c
    line_renderer: LineRenderer,

    // To render map grid
    grid_renderer: GridRenderer,
}

impl Renderer {
    pub fn new(gl: Rc<Gl>) -> Result<Renderer, JsError> {
        Ok(Renderer {
            texture_library: TextureManager::new(gl.clone())?,
            texture_renderer: TextureRenderer::new(gl.clone())?,
            line_renderer: LineRenderer::new(gl.clone())?,
            grid_renderer: GridRenderer::new(gl)?,
        })
    }

    pub fn render_grid(&mut self, vp: Rect, grid_size: f32) {
        self.grid_renderer.render_grid(vp, grid_size);
    }

    pub fn load_image(&mut self, image: &HtmlImageElement) -> u32 {
        self.texture_library.load_image(image)
    }

    pub fn draw_texture(&self, vp: Rect, texture: u32, position: Rect) {
        self.texture_renderer
            .draw_texture(vp, self.texture_library.get_texture(texture), position);
    }

    pub fn draw_outline(&mut self, vp: Rect, outline: Rect) {
        let (vp_x, vp_y, vp_w, vp_h) = vp.as_floats();
        let (x, y, w, h) = outline.as_floats();

        self.line_renderer.scale_and_load_points(
            &mut vec![
                x - vp_x,
                y - vp_y,
                x - vp_x + w,
                y - vp_y,
                x - vp_x + w,
                y - vp_y + h,
                x - vp_x,
                y - vp_y + h,
            ],
            vp_w,
            vp_h,
        );
        self.line_renderer
            .render_line_loop(Some([0.5, 0.5, 1.0, 0.9]));
    }
}

fn create_shader(gl: &Gl, src: &str, stype: u32) -> Result<WebGlShader, JsError> {
    let shader = match gl.create_shader(stype) {
        Some(s) => s,
        None => return Err(JsError::ResourceError("Failed to create shader.")),
    };

    gl.shader_source(&shader, src);
    gl.compile_shader(&shader);

    if gl
        .get_shader_parameter(&shader, Gl::COMPILE_STATUS)
        .is_falsy()
    {
        return match gl.get_shader_info_log(&shader) {
            Some(_) => Err(JsError::ResourceError("Shader compilation failed.")),
            None => Err(JsError::ResourceError(
                "Shader compilation failed, no error message.",
            )),
        };
    }

    Ok(shader)
}

fn create_program(gl: &Gl, vert: &str, frag: &str) -> Result<WebGlProgram, JsError> {
    let program = match gl.create_program() {
        Some(p) => p,
        None => return Err(JsError::ResourceError("WebGL program creation failed.")),
    };

    gl.attach_shader(&program, &create_shader(gl, vert, Gl::VERTEX_SHADER)?);
    gl.attach_shader(&program, &create_shader(gl, frag, Gl::FRAGMENT_SHADER)?);

    gl.link_program(&program);

    if gl
        .get_program_parameter(&program, Gl::LINK_STATUS)
        .is_falsy()
    {
        gl.delete_program(Some(&program));
        return Err(JsError::ResourceError("WebGL program linking failed."));
    }

    Ok(program)
}

fn create_buffer(gl: &Gl, data_opt: Option<&Float32Array>) -> Result<WebGlBuffer, JsError> {
    let buffer = match gl.create_buffer() {
        Some(b) => b,
        None => return Err(JsError::ResourceError("Failed to create WebGL buffer.")),
    };

    if let Some(data) = data_opt {
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&buffer));
        gl.buffer_data_with_opt_array_buffer(
            Gl::ARRAY_BUFFER,
            Some(&data.buffer()),
            Gl::STATIC_DRAW,
        );
    }

    Ok(buffer)
}

// see https://webglfundamentals.org/webgl/resources/m4.js
fn m4_orthographic(l: f32, r: f32, b: f32, t: f32, n: f32, f: f32) -> [f32; 16] {
    [
        2.0 / (r - l),
        0.0,
        0.0,
        0.0,
        0.0,
        2.0 / (t - b),
        0.0,
        0.0,
        0.0,
        0.0,
        2.0 / (n - f),
        0.0,
        (l + r) / (l - r),
        (b + t) / (b - t),
        (n + f) / (n - f),
        1.0,
    ]
}

// Translates matrix m by tx units in the x direction and likewise for ty and tz.
// NB: in place
fn m4_translate(m: &mut [f32; 16], tx: f32, ty: f32, tz: f32) {
    m[12] += m[0] * tx + m[4] * ty + m[8] * tz;
    m[13] += m[1] * tx + m[5] * ty + m[9] * tz;
    m[14] += m[2] * tx + m[6] * ty + m[10] * tz;
    m[15] += m[3] * tx + m[7] * ty + m[11] * tz;
}

// NB: in place
fn m4_scale(m: &mut [f32; 16], sx: f32, sy: f32, sz: f32) {
    m[0] *= sx;
    m[1] *= sx;
    m[2] *= sx;
    m[3] *= sx;
    m[4] *= sy;
    m[5] *= sy;
    m[6] *= sy;
    m[7] *= sy;
    m[8] *= sz;
    m[9] *= sz;
    m[10] *= sz;
    m[11] *= sz;
}
