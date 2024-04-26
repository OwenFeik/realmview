use scene::{Colour, PointVector};

use super::webgl::{Mesh, SolidRenderer};
use crate::scene::{Point, Rect};

pub struct FogRenderer {
    solid_renderer: SolidRenderer,
    shape: Option<Mesh>,
    current_vp: Option<Rect>,
    current_dimensions: Option<(u32, u32)>,
    current_grid_size: Option<f32>,
    current_n_revelead: Option<u32>,
}

impl FogRenderer {
    pub fn new(inner: SolidRenderer) -> Self {
        Self {
            solid_renderer: inner,
            shape: None,
            current_vp: None,
            current_dimensions: None,
            current_grid_size: None,
            current_n_revelead: None,
        }
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

        if let Ok(mut mesh) = self.solid_renderer.mesh(&points.data) {
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
