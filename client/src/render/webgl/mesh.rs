use js_sys::Float32Array;
use scene::{Point, Rect};
use web_sys::{WebGlBuffer, WebGlProgram, WebGlUniformLocation};

use super::{create_buffer, get_uniform_location, Gl};
use crate::render::shapes;

pub struct Mesh {
    coords: Float32Array,
    position_buffer: WebGlBuffer,
    position_location: u32,
    matrix_location: WebGlUniformLocation,
    vertex_count: i32,
    scale: bool,
    translate: bool,
}

impl Mesh {
    // Requires that the program use "a_position" and "u_matrix"
    pub fn new(gl: &Gl, program: &WebGlProgram, points: &[f32]) -> anyhow::Result<Self> {
        let coords = Float32Array::new_with_length(points.len() as u32);
        coords.copy_from(points);

        let position_location = gl.get_attrib_location(program, "a_position") as u32;
        let position_buffer = create_buffer(gl, Some(&coords))?;

        let matrix_location = get_uniform_location(gl, program, "u_matrix")?;

        let vertex_count = (coords.length() / 2) as i32;

        Ok(Mesh {
            coords,
            position_buffer,
            position_location,
            matrix_location,
            vertex_count,
            scale: true,
            translate: true,
        })
    }

    pub fn of_shape(gl: &Gl, program: &WebGlProgram, shape: scene::Shape) -> anyhow::Result<Self> {
        Self::new(gl, program, &shapes::shape(shape))
    }

    pub fn points(&self) -> &Float32Array {
        &self.coords
    }

    pub fn set_transforms(&mut self, scale: bool, translate: bool) {
        self.scale = scale;
        self.translate = translate;
    }

    // Should be called after using a program.
    pub fn draw(&self, gl: &Gl, vp: Rect, at: Rect) {
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.position_buffer));
        gl.enable_vertex_attrib_array(self.position_location);
        gl.vertex_attrib_pointer_with_i32(self.position_location, 2, Gl::FLOAT, false, 0, 0);

        let mut m = m4_orthographic(0.0, vp.w, vp.h, 0.0, -1.0, 1.0);

        if self.translate {
            m4_translate(&mut m, at.x - vp.x, at.y - vp.y, 0.0);
        }

        if self.scale {
            m4_scale(&mut m, at.w, at.h, 1.0);
        }

        gl.uniform_matrix4fv_with_f32_array(Some(&self.matrix_location), false, &m);
        gl.draw_arrays(Gl::TRIANGLES, 0, self.vertex_count);
    }

    pub fn draw_unscaled(&self, gl: &Gl, at: Point) {
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.position_buffer));
        gl.enable_vertex_attrib_array(self.position_location);
        gl.vertex_attrib_pointer_with_i32(self.position_location, 2, Gl::FLOAT, false, 0, 0);

        let mut m = m4_orthographic(0.0, 1.0, 1.0, 0.0, -1.0, 1.0);

        m4_translate(&mut m, at.x, at.y, 0.0);

        gl.uniform_matrix4fv_with_f32_array(Some(&self.matrix_location), false, &m);
        gl.draw_arrays(Gl::TRIANGLES, 0, self.vertex_count);
    }
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

fn m4_rotatez(m: &mut [f32; 16], theta: f32) {
    let m00 = m[0];
    let m01 = m[1];
    let m02 = m[2];
    let m03 = m[3];
    let m10 = m[4];
    let m11 = m[5];
    let m12 = m[6];
    let m13 = m[7];
    let c = theta.cos();
    let s = theta.sin();

    m[0] = c * m00 + s * m10;
    m[1] = c * m01 + s * m11;
    m[2] = c * m02 + s * m12;
    m[3] = c * m03 + s * m13;
    m[4] = c * m10 - s * m00;
    m[5] = c * m11 - s * m01;
    m[6] = c * m12 - s * m02;
    m[7] = c * m13 - s * m03;
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
