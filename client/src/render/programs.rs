use std::collections::HashMap;
use std::rc::Rc;

use scene::{Colour, PointVector};

use super::{Gl, ViewInfo};
use crate::bridge::log;
use crate::scene::{Point, Rect, Shape};

pub struct HollowRenderer {
    gl: Rc<Gl>,
    grid_size: f32,
    meshes: HashMap<scene::Id, (u8, u32, f32, f32, f32, Mesh)>, // { id: (n, stroke, rect, mesh) }
    renderer: SolidRenderer,
}

impl HollowRenderer {
    pub fn new(gl: Rc<Gl>) -> anyhow::Result<Self> {
        Ok(Self {
            gl: gl.clone(),
            grid_size: 0.0,
            meshes: HashMap::new(),
            renderer: SolidRenderer::new(gl)?,
        })
    }

    fn add_shape(
        &mut self,
        id: scene::Id,
        shape: scene::Shape,
        stroke: f32,
        viewport: Rect,
        position: Rect,
    ) -> anyhow::Result<()> {
        let points = super::shapes::hollow_shape(
            shape,
            Rect {
                x: (stroke * self.grid_size) / viewport.w,
                y: (stroke * self.grid_size) / viewport.h,
                w: position.w / viewport.w,
                h: position.h / viewport.h,
            },
        );
        let mut mesh = Mesh::new(&self.gl, &self.renderer.program, &points)?;
        mesh.set_transforms(false, true);
        self.meshes
            .insert(id, (shape as u8, 1, stroke, position.w, position.h, mesh));
        Ok(())
    }

    fn get_mesh(
        &self,
        id: scene::Id,
        shape: scene::Shape,
        points: u32,
        stroke: f32,
        rect: Rect,
    ) -> Option<&Mesh> {
        if let Some((shp, n, s, w, h, mesh)) = self.meshes.get(&id) {
            // If n is different, drawing has changed, we don't have it
            if shape as u8 == *shp && points == *n && stroke == *s && rect.w == *w && rect.h == *h {
                return Some(mesh);
            }
        }
        None
    }

    fn update_grid_size(&mut self, grid_size: f32) {
        if self.grid_size != grid_size {
            self.grid_size = grid_size;
            self.meshes.clear();
        }
    }

    pub fn draw_shape(
        &mut self,
        id: scene::Id,
        shape: scene::Shape,
        colour: Colour,
        stroke: f32,
        viewport: Rect,
        position: Rect,
        grid_size: f32,
    ) {
        let pos = position.positive_dimensions();
        self.update_grid_size(grid_size);
        if let Some(shape) = self.get_mesh(id, shape, 1, stroke, pos) {
            self.renderer.draw_unscaled(shape, colour, viewport, pos);
        } else if self.add_shape(id, shape, stroke, viewport, pos).is_ok() {
            self.draw_shape(id, shape, colour, stroke, viewport, pos, grid_size);
        }
    }
}

pub struct TextureRenderer {
    ellipse: TextureShapeRenderer,
    hexagon: TextureShapeRenderer,
    rectangle: TextureShapeRenderer,
    triangle: TextureShapeRenderer,
}

impl TextureRenderer {
    pub fn new(gl: Rc<Gl>) -> anyhow::Result<Self> {
        Ok(TextureRenderer {
            ellipse: TextureShapeRenderer::new(gl.clone(), Shape::Ellipse)?,
            hexagon: TextureShapeRenderer::new(gl.clone(), Shape::Hexagon)?,
            rectangle: TextureShapeRenderer::new(gl.clone(), Shape::Rectangle)?,
            triangle: TextureShapeRenderer::new(gl, Shape::Triangle)?,
        })
    }

    pub fn draw_texture(
        &self,
        shape: Shape,
        texture: &WebGlTexture,
        viewport: Rect,
        position: Rect,
    ) {
        match shape {
            Shape::Ellipse => self.ellipse.draw_texture(texture, viewport, position),
            Shape::Hexagon => self.hexagon.draw_texture(texture, viewport, position),
            Shape::Rectangle => self.rectangle.draw_texture(texture, viewport, position),
            Shape::Triangle => self.triangle.draw_texture(texture, viewport, position),
        }
    }
}

pub struct GridRenderer {
    line_renderer: LineRenderer,
    current_vp: Option<Rect>,
    current_grid_dims: Option<(u32, u32)>,
    current_grid_size: Option<f32>,
    current_line_count: Option<i32>,
}

impl GridRenderer {
    pub fn new(gl: Rc<Gl>) -> anyhow::Result<GridRenderer> {
        Ok(GridRenderer {
            line_renderer: LineRenderer::new(gl)?,
            current_vp: None,
            current_grid_dims: None,
            current_grid_size: None,
            current_line_count: None,
        })
    }

    pub fn create_grid(&mut self, vp: Rect, dims: (u32, u32), grid_size: f32) {
        let mut verticals = Vec::new();
        let mut horizontals = Vec::new();

        let d = grid_size;
        let dx = vp.x % grid_size;
        let dy = vp.y % grid_size;

        let w = vp.w;
        let h = vp.h;

        let (sw, sh) = dims;
        let sw = sw as f32;
        let sh = sh as f32;

        // Horizontal and vertical line start and endpoints, to ensure that we
        // render only the tiles that are part of the scene as part of the
        // grid.
        let fx = if vp.x < 0.0 {
            to_unit(vp.x.abs() / d, w / d)
        } else {
            -1.0
        };
        let tx = if (vp.x + w) / d > sw {
            to_unit(sw - vp.x / d, w / d).clamp(-1.0, 1.0)
        } else {
            1.0
        };
        let fy = if vp.y < 0.0 {
            -to_unit(vp.y.abs() / d, h / d)
        } else {
            1.0
        };
        let ty = if (vp.y + h) / d > sh {
            -to_unit(sh - vp.y / d, h / d).clamp(-1.0, 1.0)
        } else {
            -1.0
        };

        let mut i = 0.0;
        while i <= vp.w.max(vp.h) / d {
            let sx = i + (vp.x - dx) / d;
            let mut x = d * i - dx;
            if x <= w && sx >= 0.0 && sx <= sw {
                x = to_unit(x, w);

                verticals.push(x);
                verticals.push(fy);
                verticals.push(x);
                verticals.push(ty);
            }

            let sy = i + (vp.y - dy) / d;
            let mut y = d * i - dy;
            if y <= h && sy >= 0.0 && sy <= sh {
                // I negate the expression here but not for the x because the OpenGL coordinate system naturally matches
                // the browser coordinate system in the x direction, but opposes it in the y direction. By negating the
                // two coordinate systems are aligned, which makes things a little easier to work with.
                y = -to_unit(y, h);

                horizontals.push(fx);
                horizontals.push(y);
                horizontals.push(tx);
                horizontals.push(y);
            }

            i += 1.0;
        }

        verticals.append(&mut horizontals);
        self.line_renderer.load_points(&verticals);
        self.current_vp = Some(vp);
        self.current_grid_dims = Some(dims);
        self.current_grid_size = Some(grid_size);
        self.current_line_count = Some(verticals.len() as i32 / 2);
    }

    pub fn render_grid(&mut self, vp: ViewInfo, dimensions: (u32, u32)) {
        if self.current_vp.is_none()
            || self.current_vp.unwrap() != vp.viewport
            || self.current_grid_dims.is_none()
            || self.current_grid_dims.unwrap() != dimensions
            || self.current_grid_size != Some(vp.grid_size)
        {
            self.create_grid(vp.viewport, dimensions, vp.grid_size);
        }

        self.line_renderer.render_lines(None);
    }
}

pub struct FogRenderer {
    solid_renderer: SolidRenderer,
    shape: Option<Mesh>,
    current_vp: Option<Rect>,
    current_dimensions: Option<(u32, u32)>,
    current_grid_size: Option<f32>,
    current_n_revelead: Option<u32>,
}

impl FogRenderer {
    pub fn new(gl: Rc<Gl>) -> anyhow::Result<Self> {
        Ok(Self {
            solid_renderer: SolidRenderer::new(gl)?,
            shape: None,
            current_vp: None,
            current_dimensions: None,
            current_grid_size: None,
            current_n_revelead: None,
        })
    }

    pub fn create_fog(&mut self, vp: Rect, grid_size: f32, fog: &scene::Fog) {
        let mut points = PointVector::new();

        let d = grid_size;

        let mut fill_tile = |x, y| {
            let x = (x as f32) * d - vp.x;
            let y = (y as f32) * d - vp.y;

            let tl = Point::new(x, y);
            let tr = Point::new(x + d, y);
            let bl = Point::new(x, y + d);
            let br = Point::new(x + d, y + d);

            points.add_tri(tl, tr, br);
            points.add_tri(tl, bl, br);
        };

        for x in 0..fog.w {
            for y in 0..fog.h {
                if fog.occluded(x, y) {
                    fill_tile(x, y);
                }
            }
        }

        self.current_vp = Some(vp);
        self.current_dimensions = Some((fog.w, fog.h));
        self.current_grid_size = Some(grid_size);
        self.current_n_revelead = Some(fog.n_revealed);

        if let Ok(mut mesh) = Mesh::new(
            &self.solid_renderer.gl,
            &self.solid_renderer.program,
            &points.data,
        ) {
            mesh.set_transforms(false, false);
            self.shape = Some(mesh);
        }
    }

    pub fn render_fog(&mut self, vp: Rect, grid_size: f32, fog: &scene::Fog, colour: Colour) {
        if self.shape.is_none()
            || self.current_vp.is_none()
            || self.current_vp.unwrap() != vp
            || self.current_dimensions.is_none()
            || self.current_dimensions.unwrap() != (fog.w, fog.h)
            || self.current_grid_size != Some(grid_size)
            || self.current_n_revelead != Some(fog.n_revealed)
        {
            self.create_fog(vp, grid_size, fog);
        }

        if let Some(shape) = self.shape.as_ref() {
            self.solid_renderer.draw(shape, colour, vp, vp);
        }
    }
}
