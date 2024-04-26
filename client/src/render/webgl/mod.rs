use js_sys::Float32Array;
use web_sys::{
    WebGl2RenderingContext, WebGlBuffer, WebGlProgram, WebGlShader, WebGlUniformLocation,
};

mod line;
mod mesh;
mod solid;
mod texture;

pub type Gl = WebGl2RenderingContext;

pub use {
    line::LineRenderer, mesh::Mesh, solid::SolidRenderer, texture::TextureManager,
    texture::TextureShapeRenderer,
};

fn get_uniform_location(
    gl: &Gl,
    program: &WebGlProgram,
    location: &str,
) -> anyhow::Result<WebGlUniformLocation> {
    match gl.get_uniform_location(program, location) {
        Some(l) => Ok(l),
        None => Err(anyhow::anyhow!(
            "Failed to get WebGlUniformLocation {location}."
        )),
    }
}

fn create_buffer(gl: &Gl, data_opt: Option<&Float32Array>) -> anyhow::Result<WebGlBuffer> {
    let buffer = match gl.create_buffer() {
        Some(b) => b,
        None => return Err(anyhow::anyhow!("Failed to create WebGL buffer.")),
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

fn create_shader(gl: &Gl, src: &str, stype: u32) -> anyhow::Result<WebGlShader> {
    let shader = match gl.create_shader(stype) {
        Some(s) => s,
        None => return Err(anyhow::anyhow!("Failed to create shader.")),
    };

    gl.shader_source(&shader, src);
    gl.compile_shader(&shader);

    if gl
        .get_shader_parameter(&shader, Gl::COMPILE_STATUS)
        .is_falsy()
    {
        return match gl.get_shader_info_log(&shader) {
            Some(e) => Err(anyhow::anyhow!("Shader compilation failed, log: {e}")),
            None => Err(anyhow::anyhow!(
                "Shader compilation failed, no error message."
            )),
        };
    }

    Ok(shader)
}

fn create_program(gl: &Gl, vert: &str, frag: &str) -> anyhow::Result<WebGlProgram> {
    let program = match gl.create_program() {
        Some(p) => p,
        None => return Err(anyhow::anyhow!("WebGL program creation failed.")),
    };

    gl.attach_shader(&program, &create_shader(gl, vert, Gl::VERTEX_SHADER)?);
    gl.attach_shader(&program, &create_shader(gl, frag, Gl::FRAGMENT_SHADER)?);

    gl.link_program(&program);

    if gl
        .get_program_parameter(&program, Gl::LINK_STATUS)
        .is_falsy()
    {
        gl.delete_program(Some(&program));
        return Err(anyhow::anyhow!("WebGL program linking failed."));
    }

    Ok(program)
}

struct Shapes {
    ellipse: Mesh,
    hexagon: Mesh,
    rectangle: Mesh,
    triangle: Mesh,
}

impl Shapes {
    fn new(gl: &Gl, program: &WebGlProgram) -> anyhow::Result<Self> {
        Ok(Shapes {
            ellipse: Mesh::of_shape(gl, program, scene::Shape::Ellipse)?,
            hexagon: Mesh::of_shape(gl, program, scene::Shape::Hexagon)?,
            rectangle: Mesh::of_shape(gl, program, scene::Shape::Rectangle)?,
            triangle: Mesh::of_shape(gl, program, scene::Shape::Triangle)?,
        })
    }

    fn shape(&self, shape: scene::Shape) -> &Mesh {
        match shape {
            scene::Shape::Ellipse => &self.ellipse,
            scene::Shape::Hexagon => &self.hexagon,
            scene::Shape::Rectangle => &self.rectangle,
            scene::Shape::Triangle => &self.triangle,
        }
    }
}
