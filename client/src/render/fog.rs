use scene::{Colour, PointVector};

use super::webgl::{Mesh, SolidRenderer};
use crate::scene::Rect;

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

        let mut fill_tiles = |x, y, h| {
            points.add_rect(Rect {
                x: (x as f32) * d - vp.x,
                y: (y as f32) * d - vp.y,
                w: d,
                h: (h as f32) * d,
            });
        };

        let mut y_start = None;
        for x in 0..fog.w {
            for y in 0..fog.h {
                if fog.occluded(x, y) {
                    if y_start.is_none() {
                        y_start = Some(y);
                    }
                } else {
                    if let Some(y_start) = y_start {
                        fill_tiles(x, y_start, y - y_start);
                    }
                    y_start = None;
                }
            }
            if let Some(y_start) = y_start {
                fill_tiles(x, y_start, fog.h - y_start);
            }
            y_start = None;
        }

        let grid_w = fog.w as f32 * grid_size;
        let grid_h = fog.h as f32 * grid_size;

        if vp.x < 0.0 {
            points.add_rect(Rect {
                x: 0.0,
                y: if vp.y < 0.0 { vp.y.abs() } else { 0.0 },
                w: if vp.x < 0.0 { vp.x.abs() } else { 0.0 },
                h: vp.h,
            });
        }

        if vp.y < 0.0 {
            points.add_rect(Rect {
                x: 0.0,
                y: 0.0,
                w: grid_w - vp.x,
                h: -vp.y,
            });
        }

        if vp.x + vp.w > grid_w {
            points.add_rect(Rect {
                x: grid_w - vp.x,
                y: 0.0,
                w: vp.w - (grid_w - vp.x),
                h: grid_h - vp.y,
            })
        }

        if vp.y + vp.h > grid_h {
            points.add_rect(Rect {
                x: -vp.x,
                y: grid_h - vp.y,
                w: vp.w + vp.x,
                h: vp.h - (grid_h - vp.y),
            })
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
