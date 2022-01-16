use std::rc::Rc;

use js_sys::Float32Array;
use web_sys::{WebGlBuffer, WebGlProgram, WebGlShader, WebGlTexture, WebGlUniformLocation};

use crate::bridge::{Gl, JsError};
use crate::scene::Rect;


pub struct TextureRenderer {
    gl: Rc<Gl>,
    program: WebGlProgram,
    position_buffer: WebGlBuffer,
    position_location: u32,
    texcoord_buffer: WebGlBuffer,
    texcoord_location: u32,
    matrix_location: WebGlUniformLocation,
    texture_location: WebGlUniformLocation
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

        // Just a square. A 4x4 matrix is used to transform this to the appropriate dimensions when rendering textures.
        let coords = Float32Array::new_with_length(12);
        coords.copy_from(&[
            0.0, 0.0,
            0.0, 1.0,
            1.0, 0.0,
            1.0, 0.0,
            0.0, 1.0,
            1.0, 1.0
        ]);
        let position_buffer = create_buffer(&gl, Some(&coords))?;
        let texcoord_buffer = create_buffer(&gl, Some(&coords))?;

        let matrix_location = match gl.get_uniform_location(&program, "u_matrix") {
            Some(p) => p,
            None => return Err(JsError::ResourceError("Couldn't find shader matrix location."))
        };
        let texture_location = match gl.get_uniform_location(&program, "u_texture") {
            Some(l) => l,
            None => return Err(JsError::ResourceError("Couldn't find texture matrix location."))
        };

        Ok(
            TextureRenderer {
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

    pub fn draw_texture(&self, viewport: &Rect, texture: &WebGlTexture, position: &Rect) {
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
        m4_translate(&mut matrix, (position.x - viewport.x) as f32, (position.y - viewport.y) as f32, 0.0);
        m4_scale(&mut matrix, position.w as f32, position.h as f32, 1.0);

        gl.uniform_matrix4fv_with_f32_array(Some(&self.matrix_location), false, &matrix);
        gl.uniform1i(Some(&self.texture_location), 0);
        gl.draw_arrays(Gl::TRIANGLES, 0, 6);
    }
}


pub struct GridRenderer {
    gl: Rc<Gl>,
    program: WebGlProgram,
    position_location: u32,
    position_buffer: WebGlBuffer,
    current_grid_rect: Option<Rect>,
    current_grid_size: Option<i32>,
    current_line_count: Option<i32>
}


const GRID_VERTEX_SHADER: &str = "
attribute vec4 a_position;

void main() {
    gl_Position = a_position;
}
";


const GRID_FRAGMENT_SHADER: &str = "
precision mediump float;

void main() {
  gl_FragColor = vec4(0.5, 0.5, 0.5, 0.75);
}
";


impl GridRenderer {
    pub fn new(gl: Rc<Gl>) -> Result<GridRenderer, JsError> {
        let program = create_program(&gl, GRID_VERTEX_SHADER, GRID_FRAGMENT_SHADER)?;
        let position_location = gl.get_attrib_location(&program, "a_position") as u32;
        let position_buffer = create_buffer(&gl, None)?;
        Ok(GridRenderer {
            gl,
            program,
            position_location,
            position_buffer,
            current_grid_rect: None,
            current_grid_size: None,
            current_line_count: None
        })
    }

    pub fn create_grid(&mut self, vp: Rect, grid_size: i32) {
        let mut verticals = Vec::new();
        let mut horizontals = Vec::new();

        let d = grid_size as f32;
        let dx = (vp.x % grid_size) as f32;
        let dy = (vp.y % grid_size) as f32;

        let w = vp.w as f32;
        let h = vp.h as f32;

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
                y = (2.0 * y - h) / h; // Map to [-1, 1]

                horizontals.push(-1.0);
                horizontals.push(y);
                horizontals.push(1.0);
                horizontals.push(y);
                finished = false;
            }

            i += 1.0;
        }

        verticals.append(&mut horizontals);


        let positions = Float32Array::from(&verticals[..]);
        
        self.gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.position_buffer));
        self.gl.buffer_data_with_opt_array_buffer(Gl::ARRAY_BUFFER, Some(&positions.buffer()), Gl::STATIC_DRAW);

        self.current_grid_rect = Some(vp);
        self.current_grid_size = Some(grid_size);
        self.current_line_count = Some(verticals.len() as i32 / 2);
    }

    pub fn render_grid(&mut self, vp: Rect, grid_size: i32) {
        if let Some(rect) = &self.current_grid_rect {
            if *rect != vp || self.current_grid_size != Some(grid_size) {
                self.create_grid(vp, grid_size);
            }
        }
        else { self.create_grid(vp, grid_size); }

        let gl = &self.gl;

        gl.use_program(Some(&self.program));
        gl.enable_vertex_attrib_array(self.position_location);
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.position_buffer));
        gl.vertex_attrib_pointer_with_i32(self.position_location, 2, Gl::FLOAT, false, 0, 0);
        gl.draw_arrays(Gl::LINES, 0, self.current_line_count.unwrap_or(0));
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


fn create_program(gl: &Gl, vert: &str, frag: &str) -> Result<WebGlProgram, JsError> {
    let program = match gl.create_program() {
        Some(p) => p,
        None => return Err(JsError::ResourceError("WebGL program creation failed."))
    };

    gl.attach_shader(&program, &create_shader(&gl, vert, Gl::VERTEX_SHADER)?);
    gl.attach_shader(&program, &create_shader(&gl, frag, Gl::FRAGMENT_SHADER)?);

    gl.link_program(&program);

    if gl.get_program_parameter(&program, Gl::LINK_STATUS).is_falsy() {
        gl.delete_program(Some(&program));
        return Err(JsError::ResourceError("WebGL program linking failed."));
    }

    Ok(program)
}


fn create_buffer(gl: &Gl, data_opt: Option<&Float32Array>) -> Result<WebGlBuffer, JsError> {
    let buffer = match gl.create_buffer() {
        Some(b) => b,
        None => return Err(JsError::ResourceError("Failed to create WebGL buffer."))
    };

    if let Some(data) = data_opt {
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&buffer));
        gl.buffer_data_with_opt_array_buffer(
            Gl::ARRAY_BUFFER,
            Some(&data.buffer()),
            Gl::STATIC_DRAW
        );    
    }

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
