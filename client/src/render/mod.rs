use std::rc::Rc;

use web_sys::{HtmlImageElement, WebGl2RenderingContext};

use scene::{Rect, Sprite, SpriteVisual};

mod programs;
mod shapes;
pub struct Renderer {
    // Loads and stores references to textures
    texture_library: programs::TextureManager,

    // Draw solid shapes
    solid_renderer: programs::SolidRenderer,

    // Draw textures in shapes
    texture_renderer: programs::TextureRenderer,

    // To render outlines &c
    line_renderer: programs::LineRenderer,

    // To render map grid
    grid_renderer: programs::GridRenderer,
}

impl Renderer {
    pub fn new(gl: Rc<WebGl2RenderingContext>) -> anyhow::Result<Self> {
        Ok(Renderer {
            texture_library: programs::TextureManager::new(gl.clone())?,
            solid_renderer: programs::SolidRenderer::new(gl.clone())?,
            texture_renderer: programs::TextureRenderer::new(gl.clone())?,
            line_renderer: programs::LineRenderer::new(gl.clone())?,
            grid_renderer: programs::GridRenderer::new(gl)?,
        })
    }

    pub fn render_grid(&mut self, vp: Rect, dims: Rect, grid_size: f32) {
        self.grid_renderer.render_grid(vp, dims, grid_size);
    }

    pub fn load_image(&mut self, image: &HtmlImageElement) -> scene::Id {
        self.texture_library.load_image(image)
    }

    pub fn draw_sprite(&mut self, sprite: &Sprite, viewport: Rect, grid_size: f32) {
        let position = sprite.rect * grid_size;
        match &sprite.visual {
            SpriteVisual::Solid { colour, shape, .. } => self
                .solid_renderer
                .draw_shape(*shape, *colour, viewport, position),
            SpriteVisual::Texture { id, shape } => self.texture_renderer.draw_texture(
                *shape,
                self.texture_library.get_texture(*id),
                viewport,
                position,
            ),
            SpriteVisual::Drawing {
                stroke,
                drawing,
                colour,
            } => {
                self.line_renderer.load_points(&shapes::drawing(
                    &drawing.points,
                    *stroke,
                    drawing.cap_start,
                    drawing.cap_end,
                ));
                self.line_renderer.render_solid(Some(*colour));
            }
        }
    }

    pub fn draw_outline(
        &mut self,
        Rect {
            x: vp_x,
            y: vp_y,
            w: vp_w,
            h: vp_h,
        }: Rect,
        Rect { x, y, w, h }: Rect,
    ) {
        self.line_renderer.scale_and_load_points(
            &mut [
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

/// Parses a 16 digit hexadecimal media key string into an Id, reutrning 0
/// on failure.
pub fn parse_media_key(key: &str) -> scene::Id {
    if key.len() != 16 {
        return 0;
    }

    let mut raw = [0; 8];
    for (i, r) in raw.iter_mut().enumerate() {
        let j = i * 2;
        if let Ok(b) = u8::from_str_radix(&key[j..j + 2], 16) {
            *r = b;
        } else {
            return 0;
        }
    }

    i64::from_be_bytes(raw)
}
