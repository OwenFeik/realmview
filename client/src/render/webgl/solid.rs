use std::rc::Rc;

use scene::{Colour, Point, Rect};
use web_sys::{WebGlProgram, WebGlUniformLocation};

use super::{create_program, get_uniform_location, mesh::Mesh, Gl, Shapes};

pub struct SolidRenderer {
    gl: Rc<Gl>,
    program: WebGlProgram,
    colour_location: WebGlUniformLocation,
    shapes: Shapes,
}

impl SolidRenderer {
    pub fn new(gl: Rc<Gl>) -> anyhow::Result<Self> {
        let program = create_program(
            &gl,
            include_str!("shaders/solid.vert"),
            include_str!("shaders/single.frag"),
        )?;

        let colour_location = get_uniform_location(&gl, &program, "u_colour")?;
        let shapes = Shapes::new(&gl, &program)?;

        Ok(SolidRenderer {
            gl,
            program,
            colour_location,
            shapes,
        })
    }

    pub fn mesh(&self, points: &[f32]) -> anyhow::Result<Mesh> {
        Mesh::new(&self.gl, &self.program, points)
    }

    fn prepare_draw(&self, colour: Colour) {
        self.gl.use_program(Some(&self.program));
        self.gl
            .uniform4fv_with_f32_array(Some(&self.colour_location), colour.arr());
    }

    pub fn draw(&self, shape: &Mesh, colour: Colour, viewport: Rect, position: Rect) {
        self.prepare_draw(colour);
        shape.draw(&self.gl, viewport, position);
    }

    pub fn draw_unscaled(&self, shape: &Mesh, colour: Colour, vp: Rect, position: Rect) {
        self.prepare_draw(colour);
        shape.draw_unscaled(
            &self.gl,
            (position.top_left() - vp.top_left()) / Point::new(vp.w, vp.h),
        );
    }

    pub fn draw_shape(&self, shape: scene::Shape, colour: Colour, viewport: Rect, position: Rect) {
        self.draw(self.shapes.shape(shape), colour, viewport, position);
    }
}
