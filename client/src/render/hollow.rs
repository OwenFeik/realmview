use std::collections::HashMap;

use scene::{Colour, Rect};

use super::webgl::{Mesh, SolidRenderer};

pub struct HollowRenderer {
    grid_size: f32,
    meshes: HashMap<scene::Id, (u8, u32, f32, f32, f32, Mesh)>, // { id: (n, stroke, rect, mesh) }
    renderer: SolidRenderer,
}

impl HollowRenderer {
    pub fn new(inner: SolidRenderer) -> Self {
        Self {
            grid_size: 0.0,
            meshes: HashMap::new(),
            renderer: inner,
        }
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
        let mut mesh = self.renderer.mesh(&points)?;
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
