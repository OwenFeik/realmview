use std::rc::Rc;

use scene::{
    Cap, Colour, Drawing, DrawingMode, Fog, Id, Outline, Point, Rect, Scene, Shape, Sprite,
};
use web_sys::{HtmlImageElement, WebGl2RenderingContext};

use crate::viewport::ViewportPoint;

mod programs;
mod shapes;
mod text;

#[derive(Clone, Copy, Debug)]
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

    pub fn viewport_point(&self, scene_point: Point) -> ViewportPoint {
        let point = scene_point * self.grid_size;
        ViewportPoint {
            x: point.x - self.viewport.x,
            y: point.y - self.viewport.y,
        }
    }
}

pub trait Renderer {
    /// Clear the canvas.
    ///
    /// * `vp` Viewport position and dimensions, tile size in pixels.
    fn clear(&mut self, vp: ViewInfo);

    /// Draw a grid of a given size. Assume (0, 0) in scene space is the top
    /// left corner of the grid and each tile should be the size given the the
    /// `ViewInfo`.
    ///
    /// * `vp`         Viewport position and dimensions, tile size in pixels.
    /// * `dimensions` `(width, height)` of grid.
    fn draw_grid(&mut self, vp: ViewInfo, dimensions: (u32, u32));

    /// Render scene fog over the grid. This should be called after all sprites
    /// and the grid are rendered. The size of the fog should be the same size
    /// as the size passed to `draw_grid`. If `transparent` is false, the fog
    /// will be rendered opaque, otherwise it will have transparency.
    ///
    /// * `vp`          Viewport position and dimensions, tile size in pixels.
    /// * `fog`         Reference to scene's `Fog` struct.
    /// * `transparent` `false` to render fog as opaque.
    fn draw_fog(&mut self, vp: ViewInfo, fog: &Fog, transparent: bool);

    /// Draw a solid shape of a given colour onto the grid at a given position.
    ///
    /// * `vp`       Viewport position and dimensions, tile size in pixels.
    /// * `position` Position and dimensions of the shape, in scene units.
    /// * `shape`    Shape to draw.
    /// * `colour`   Colour to draw shape in. May be transparent.
    fn draw_solid(&mut self, vp: ViewInfo, position: Rect, shape: Shape, colour: Colour);

    /// Draw a hollow shape of a given colour with a given stroke width onto
    /// the grid at a given position.
    ///
    /// * `vp`       Viewport position and dimensions, tile size in pixels.
    /// * `position` Position and dimensions of the shape, in scene units.
    /// * `shape`    Shape to draw.
    /// * `colour`   Colour to draw shape in. May be transparent.
    /// * `stroke`   Width of the border of the hollow shape in scene units.
    fn draw_hollow(
        &mut self,
        vp: ViewInfo,
        position: Rect,
        shape: Shape,
        colour: Colour,
        stroke: f32,
    );

    /// Draw a one-pixel in a given shape at a given position.
    ///
    /// * `vp`       Viewport position and dimensions, tile size in pixels.
    /// * `position` Position and dimensions of the shape, in scene units.
    /// * `shape`    Shape to outline.
    /// * `colour`   Colour to draw outline in. May be transparent.
    fn draw_outline(&mut self, vp: ViewInfo, position: Rect, shape: Shape, colour: Colour);

    /// Draw a texture from the texture library at a given position and bounded
    /// by a given shape. If the texture is missing from the library, the
    /// default missing texture will be rendered instead.
    ///
    /// * `vp`       Viewport position and dimensions, tile size in pixels.
    /// * `position` Position and dimensions of the shape, in scene units.
    /// * `shape`    Shape to form bounds of the texture.
    /// * `texture`  ID of the texture to render.
    fn draw_texture(&mut self, vp: ViewInfo, position: Rect, shape: Shape, texture: Id);

    /// Draw a drawing at a given position. Start cap will be rendered pointing
    /// away from the angle formed by the first two points and end cap will be
    /// rendered pointing in the angle formed by the last two points.
    ///
    /// * `vp`       Viewport position and dimensions, tile size in pixels.
    /// * `position` Bounding box of the drawing.
    /// * `drawing`  Drawing to render in bounding box.
    /// * `mode`     If `Line`, a line from first to last, else all points.
    /// * `colour`   Colour to render line in.
    /// * `stroke`   Width of line, in scene units.
    /// * `start`    Cap to render at start of line.
    /// * `end`      Cap to render at end of line.
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

    /// Draw a collection of rectangular outlines onto the canvas.
    ///
    /// * `vp`       Viewport position and dimensions, tile size in pixels.
    /// * `outlines` Outlines to draw onto the grid.
    fn draw_outlines(&mut self, vp: ViewInfo, outlines: &[Outline]) {
        const PALE_BLUE_OUTLINE: Colour = Colour([0.5, 0.5, 1.0, 0.9]);

        for &Outline { rect, shape } in outlines {
            self.draw_outline(vp, rect, shape, PALE_BLUE_OUTLINE);
        }
    }

    /// Draw a sprite onto the grid, using the appropriate primitives. Places
    /// the sprite at the scene position indicated by its `rect` field. If the
    /// sprite has a `Visual::Drawing` and `drawing` is `None`, nothing will be
    /// rendered.
    ///
    /// * `vp`      Viewport position and dimensions, tile size in pixels.
    /// * `sprite`  Sprite to draw onto the grid.
    /// * `drawing` Drawing which is the sprite's visual, if applicable.
    fn draw_sprite(&mut self, vp: ViewInfo, sprite: &Sprite, drawing: Option<&Drawing>) {
        let position = sprite.rect;
        match sprite.visual {
            scene::SpriteVisual::Texture { shape, id } => {
                self.draw_texture(vp, position, shape, id)
            }
            scene::SpriteVisual::Shape {
                shape,
                stroke,
                solid: _,
                colour,
            } => {
                if sprite.visual.is_solid() {
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

    /// Draw a view of `scene` onto the canvas, with viewport dimensions and
    /// tile size as specified by `vp`.
    ///
    /// * `vp`    Viewport position and dimensions, tile size in pixels.
    /// * `scene` Scene to render view of.
    fn draw_scene(&mut self, vp: ViewInfo, scene: &Scene) {
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

    /// Draw hover text bubble at
    fn draw_text(&mut self, vp: ViewInfo, at: Point, text: &str);
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
    text_manager: text::HoverTextManager,
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
            text_manager: text::HoverTextManager::new(),
        })
    }

    pub fn load_image(&mut self, image: &HtmlImageElement) -> scene::Id {
        self.texture_library.load_image(image)
    }
}

impl Renderer for WebGlRenderer {
    fn clear(&mut self, vp: ViewInfo) {
        self.gl
            .viewport(0, 0, vp.viewport.w as i32, vp.viewport.h as i32);
        self.gl.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
        self.text_manager.clear();
    }

    fn draw_grid(&mut self, vp: ViewInfo, dimensions: (u32, u32)) {
        self.grid_renderer.render_grid(vp, dimensions);
    }

    fn draw_fog(&mut self, vp: ViewInfo, fog: &Fog, transparent: bool) {
        const TRANSPARENT_FOG_OPACITY: f32 = 0.4;

        let colour = Colour(if transparent {
            [0.0, 0.0, 0.0, TRANSPARENT_FOG_OPACITY]
        } else {
            [0.0, 0.0, 0.0, 1.0]
        });

        self.fog_renderer
            .render_fog(vp.viewport, vp.grid_size, fog, colour);
    }

    fn draw_solid(&mut self, vp: ViewInfo, position: Rect, shape: Shape, colour: Colour) {
        self.solid_renderer
            .draw_shape(shape, colour, vp.viewport, position * vp.grid_size);
    }

    fn draw_hollow(
        &mut self,
        vp: ViewInfo,
        position: Rect,
        shape: Shape,
        colour: Colour,
        stroke: f32,
    ) {
        self.hollow_renderer.draw_shape(
            0,
            shape,
            colour,
            stroke,
            vp.viewport,
            position * vp.grid_size,
            vp.grid_size,
        );
    }

    fn draw_outline(&mut self, vp: ViewInfo, position: Rect, shape: Shape, colour: Colour) {
        let Rect {
            x: vp_x,
            y: vp_y,
            w: vp_w,
            h: vp_h,
        } = vp.viewport;
        let mut points = shapes::outline_shape(
            shape,
            position
                .scaled(vp.grid_size)
                .translate(-Point::new(vp_x, vp_y)),
        );
        self.line_renderer
            .scale_and_load_points(&mut points, vp_w, vp_h);
        self.line_renderer.render_line_loop(Some(colour));
    }

    fn draw_texture(&mut self, vp: ViewInfo, position: Rect, shape: Shape, texture: Id) {
        let texture = self.texture_library.get_texture(texture);
        self.texture_renderer.draw_texture(
            shape,
            texture,
            vp.viewport,
            position.scaled(vp.grid_size),
        );
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

    fn draw_text(&mut self, vp: ViewInfo, at: Point, text: &str) {
        self.text_manager.render(vp.viewport_point(at), text);
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
