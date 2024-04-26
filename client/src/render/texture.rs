use scene::{Rect, Shape};

use super::webgl::{Texture, TextureShapeRenderer};

pub struct TextureRenderer {
    ellipse: TextureShapeRenderer,
    hexagon: TextureShapeRenderer,
    rectangle: TextureShapeRenderer,
    triangle: TextureShapeRenderer,
}

impl TextureRenderer {
    pub fn new(gl: std::rc::Rc<super::webgl::Gl>) -> anyhow::Result<Self> {
        Ok(TextureRenderer {
            ellipse: TextureShapeRenderer::new(gl.clone(), Shape::Ellipse)?,
            hexagon: TextureShapeRenderer::new(gl.clone(), Shape::Hexagon)?,
            rectangle: TextureShapeRenderer::new(gl.clone(), Shape::Rectangle)?,
            triangle: TextureShapeRenderer::new(gl, Shape::Triangle)?,
        })
    }

    pub fn draw_texture(&self, shape: Shape, texture: Texture, viewport: Rect, position: Rect) {
        match shape {
            Shape::Ellipse => self.ellipse.draw_texture(texture, viewport, position),
            Shape::Hexagon => self.hexagon.draw_texture(texture, viewport, position),
            Shape::Rectangle => self.rectangle.draw_texture(texture, viewport, position),
            Shape::Triangle => self.triangle.draw_texture(texture, viewport, position),
        }
    }
}
