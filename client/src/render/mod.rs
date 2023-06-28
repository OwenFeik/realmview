use std::rc::Rc;

use scene::{Cap, Colour, Drawing, DrawingMode, Fog, Id, Point, Rect, Scene, Shape, Sprite};
use web_sys::{HtmlImageElement, WebGl2RenderingContext};

mod programs;
mod shapes;

#[derive(Clone, Copy)]
pub struct ViewInfo {
    viewport: Rect,
    grid_size: f32,
}

impl ViewInfo {
    pub fn new(viewport: Rect, grid_size: f32) -> Self {
        Self {
            viewport,
            grid_size,
        }
    }
}

pub trait Renderer {
    fn clear(&mut self, vp: ViewInfo);

    fn draw_grid(&mut self, vp: ViewInfo, dimensions: (u32, u32));

    fn draw_fog(&mut self, vp: ViewInfo, fog: &Fog, transparent: bool);

    fn draw_solid(&mut self, vp: ViewInfo, position: Rect, shape: Shape, colour: Colour);

    fn draw_hollow(&mut self, vp: ViewInfo, position: Rect, shape: Shape, colour: Colour, stroke: f32);

    fn draw_outline(&mut self, vp: ViewInfo, position: Rect, shape: Shape, colour: Colour);

    fn draw_texture(&mut self, vp: ViewInfo, position: Rect, shape: Shape, texture: Id);

    fn draw_drawing(
        &mut self,
        vp: ViewInfo,
        position: Rect,
        drawing: &Drawing,
        mode: DrawingMode,
        colour: Colour,
        stroke: f32,
        start: Cap,
        end: Cap,
    );

    fn draw_outlines(&mut self, vp: ViewInfo, outlines: &[Rect]) {
        const PALE_BLUE_OUTLINE: Colour = Colour([0.5, 0.5, 1.0, 0.9]);

        for rect in outlines {
            self.draw_outline(
                vp,
                rect.scaled(vp.grid_size),
                Shape::Rectangle,
                PALE_BLUE_OUTLINE,
            );
        }
    }

    fn draw_sprite(&mut self, vp: ViewInfo, sprite: &Sprite, drawing: Option<&Drawing>) {
        let position = sprite.rect;
        match sprite.visual {
            scene::SpriteVisual::Texture { shape, id } => {
                self.draw_texture(vp, position, shape, id)
            }
            scene::SpriteVisual::Shape {
                shape,
                stroke,
                colour,
            } => {
                if stroke <= f32::EPSILON {
                    self.draw_solid(vp, position, shape, colour);                    
                } else {
                    self.draw_hollow(vp, position, shape, colour, stroke);
                }
            }
            scene::SpriteVisual::Drawing {
                drawing: _id,
                mode,
                colour,
                stroke,
                cap_start,
                cap_end,
            } => {
                if let Some(drawing) = drawing {
                    self.draw_drawing(
                        vp, position, drawing, mode, colour, stroke, cap_start, cap_end,
                    );
                }
            }
        }
    }

    fn draw_scene(&mut self, vp: ViewInfo, scene: &Scene, ) {
        let dimensions = (scene.w(), scene.h());
        
        let mut background_drawn = false;
        for layer in scene.layers.iter().rev() {
            if !background_drawn && layer.z >= 0 {
                self.draw_grid(vp, dimensions);
                background_drawn = true;
            }

            if layer.visible {
                for sprite in &layer.sprites {
                    let drawing = sprite.visual.drawing().and_then(|id| scene.get_drawing(id));
                    self.draw_sprite(vp, sprite, drawing);
                }
            }
        }

        if !background_drawn {
            self.draw_grid(vp, dimensions);
        }
    }
}

pub struct WebGlRenderer {
    gl: Rc<WebGl2RenderingContext>,
    texture_library: programs::TextureManager,
    solid_renderer: programs::SolidRenderer,
    texture_renderer: programs::TextureRenderer,
    hollow_renderer: programs::HollowRenderer,
    drawing_renderer: programs::DrawingRenderer,
    line_renderer: programs::LineRenderer,
    grid_renderer: programs::GridRenderer,
    fog_renderer: programs::FogRenderer,
}

impl WebGlRenderer {
    pub fn new(gl: Rc<WebGl2RenderingContext>) -> anyhow::Result<Self> {
        Ok(Self {
            gl: gl.clone(),
            texture_library: programs::TextureManager::new(gl.clone())?,
            solid_renderer: programs::SolidRenderer::new(gl.clone())?,
            texture_renderer: programs::TextureRenderer::new(gl.clone())?,
            hollow_renderer: programs::HollowRenderer::new(gl.clone())?,
            drawing_renderer: programs::DrawingRenderer::new(gl.clone())?,
            line_renderer: programs::LineRenderer::new(gl.clone())?,
            grid_renderer: programs::GridRenderer::new(gl.clone())?,
            fog_renderer: programs::FogRenderer::new(gl)?,
        })
    }

    pub fn load_image(&mut self, image: &HtmlImageElement) -> scene::Id {
        self.texture_library.load_image(image)
    }
}

impl Renderer for WebGlRenderer {
    fn clear(&mut self, vp: ViewInfo) {
        programs::clear_canvas(
            &self.gl,
            vp.viewport.w * vp.grid_size,
            vp.viewport.h * vp.grid_size,
        );
    }

    fn draw_grid(&mut self, vp: ViewInfo, dimensions: (u32, u32)) {
        self.grid_renderer.render_grid(vp, dimensions);
    }

    fn draw_fog(&mut self, vp: ViewInfo, fog: &Fog, transparent: bool) {
        let colour = if transparent {
            [0.0, 0.0, 0.0, 0.5]
        } else {
            [0.0, 0.0, 0.0, 1.0]
        };

        self.fog_renderer
            .render_fog(vp.viewport, vp.grid_size, fog, colour);
    }

    fn draw_solid(&mut self, vp: ViewInfo, position: Rect, shape: Shape, colour: Colour) {
        self.solid_renderer
            .draw_shape(shape, colour.raw(), vp.viewport, position);
    }

    fn draw_hollow(&mut self, vp: ViewInfo, position: Rect, shape: Shape, colour: Colour, stroke: f32) {
        self.hollow_renderer.draw_shape(0, shape, colour.raw(), stroke, vp.viewport, position, vp.grid_size);
    }

    fn draw_outline(&mut self, vp: ViewInfo, position: Rect, shape: Shape, colour: Colour) {
        let Rect {
            x: vp_x,
            y: vp_y,
            w: vp_w,
            h: vp_h,
        } = vp.viewport;
        let mut points = shapes::outline_shape(shape, position.translate(-Point::new(vp_x, vp_y)));
        self.line_renderer
            .scale_and_load_points(&mut points, vp_w, vp_h);
        self.line_renderer.render_line_loop(Some(colour.raw()));
    }

    fn draw_texture(&mut self, vp: ViewInfo, position: Rect, shape: Shape, texture: Id) {
        let texture = self.texture_library.get_texture(texture);
        self.texture_renderer
            .draw_texture(shape, texture, vp.viewport, position);
    }

    fn draw_drawing(
        &mut self,
        vp: ViewInfo,
        position: Rect,
        drawing: &Drawing,
        mode: DrawingMode,
        colour: Colour,
        stroke: f32,
        start: Cap,
        end: Cap,
    ) {
        self.drawing_renderer.draw_drawing(
            mode,
            drawing,
            stroke,
            start,
            end,
            colour,
            vp.viewport,
            position,
            vp.grid_size,
        );
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
