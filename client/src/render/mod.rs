use std::rc::Rc;

use scene::{Cap, Colour, Drawing, DrawingMode, Fog, Id, Rect, Shape, Sprite, Scene};
use web_sys::{HtmlImageElement, WebGl2RenderingContext};

mod programs;
mod shapes;

pub struct ViewInfo {
    viewport: Rect,
    grid_size: f32
}

pub trait Renderer {
    fn clear(&mut self);

    fn draw_grid(&mut self, vp: ViewInfo, dimensions: Rect);

    fn draw_fog(&mut self, vp: ViewInfo, fog: &Fog, transparent: bool);

    fn draw_solid(&mut self, vp: ViewInfo, position: Rect, shape: Shape, colour: Colour);

    fn draw_outline(&mut self, vp: ViewInfo, position: Rect, shape: Shape, colour: Colour, stroke: f32);

    fn draw_texture(&mut self, vp: ViewInfo, position: Rect, shape: Shape, texture: Id);

    fn draw_drawing(&mut self, vp: ViewInfo, position: Rect, drawing: Drawing, mode: DrawingMode, colour: Colour, stroke: f32,  start: Cap, end: Cap);

    fn draw_scene(scene: &Scene) {
        
    }
}

pub struct WebGlRenderer {
    sprite_renderer: programs::SpriteRenderer,

    // To render outlines &c
    line_renderer: programs::LineRenderer,

    // To render map grid
    grid_renderer: programs::GridRenderer,

    // To render fog of war
    fog_renderer: programs::FogRenderer,
}

impl WebGlRenderer {
    pub fn new(gl: Rc<WebGl2RenderingContext>) -> anyhow::Result<Self> {
        Ok(Self {
            sprite_renderer: programs::SpriteRenderer::new(gl.clone())?,
            line_renderer: programs::LineRenderer::new(gl.clone())?,
            grid_renderer: programs::GridRenderer::new(gl.clone())?,
            fog_renderer: programs::FogRenderer::new(gl)?,
        })
    }

    pub fn load_image(&mut self, image: &HtmlImageElement) -> scene::Id {
        self.sprite_renderer.load_image(image)
    }

    pub fn draw_sprite(
        &mut self,
        sprite: &Sprite,
        drawing: Option<&scene::Drawing>,
        viewport: Rect,
        grid_size: f32,
    ) {
        self.sprite_renderer
            .draw_sprite(sprite, viewport, grid_size, drawing);
    }
}

impl Renderer for WebGlRenderer {
    fn draw_grid(&mut self, vp: ViewInfo, dimensions: Rect) {
        self.grid_renderer.render_grid(vp.viewport, dimensions, vp.grid_size);
    }

    fn draw_fog(&mut self, vp: ViewInfo, fog: &Fog, transparent: bool) {
        let colour = if transparent {
            [0.0, 0.0, 0.0, 0.5]
        } else {
            [0.0, 0.0, 0.0, 1.0]
        };

        self.fog_renderer.render_fog(vp.viewport, vp.grid_size, fog, colour);
    }

    fn draw_solid(&mut self, vp: ViewInfo, position: Rect, shape: Shape, colour: Colour) {
        
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

/// Parses a 16 digit hexadecimal media key string into an Id, returning 0
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
