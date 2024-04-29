use std::collections::HashMap;

use scene::Rect;

use super::webgl::{Mesh, SolidRenderer};
use crate::Res;

pub struct DrawingRenderer {
    grid_size: f32,
    drawings: HashMap<i64, Vec<(u64, Mesh)>>, //  { drawing_id: [(key, mesh)] }
    renderer: SolidRenderer,
}

impl DrawingRenderer {
    // Maximum number of distinct meshes to keep for a single drawing.
    const DRAWING_MAX_MESHES: usize = 16;

    pub fn new(inner: SolidRenderer) -> Self {
        Self {
            grid_size: 0.0,
            drawings: HashMap::new(),
            renderer: inner,
        }
    }

    fn create_key(
        rect: Rect,
        drawing: &scene::Drawing,
        stroke: f32,
        cap_start: scene::Cap,
        cap_end: scene::Cap,
    ) -> u64 {
        // Key format is a u64 with the following structure:
        //
        // 8 bits for the rect width
        // 8 bits for the rect height
        // 12 bits for the stroke width
        // 32 bits counting the number of points in the drawing
        // 2 bits for the starting cap
        // 2 bits for the ending cap
        //
        // Like so:
        // WIDTH000HEIGHT00STROKE000000N_POINTS000000000000000000000000CSCE
        //
        // Is this grotesquely overcomplicated? Yes.
        let mut key = 0u64;

        key |= ((rect.w.to_bits() << 7) >> 24) as u64;
        key <<= 8;

        key |= ((rect.h.to_bits() << 7) >> 24) as u64;
        key <<= 8;

        key |= ((stroke.to_bits() << 7) >> 20) as u64;
        key <<= 12;

        key |= drawing.n_points() as u64;
        key <<= 32;

        key |= cap_start as u64;
        key <<= 2;

        key |= cap_end as u64; // last 2 bits

        key
    }

    fn add_drawing(
        &mut self,
        position: scene::Rect,
        drawing: &scene::Drawing,
        stroke: f32,
        cap_start: scene::Cap,
        cap_end: scene::Cap,
    ) -> Res<()> {
        let mut points = match drawing.mode {
            scene::DrawingMode::Freehand => {
                // Unwrap safe as a freehand drawing must always have a
                // (possibly empty) set of points.
                let mut points = drawing.points().unwrap().clone();

                // Transform the points based on the transformation applied to
                // the sprite's rect.
                let drawing_rect = drawing.rect();
                points.translate(-drawing_rect.top_left());
                points.scale_asymmetric(position.w / drawing_rect.w, position.h / drawing_rect.h);

                super::shapes::freehand(&points, stroke, cap_start, cap_end)
            }
            scene::DrawingMode::Line => {
                super::shapes::line(drawing.line(), stroke, cap_start, cap_end)
            }
            scene::DrawingMode::Cone => super::shapes::cone(drawing.line()),
        };

        points.scale(self.grid_size);
        let mut mesh = self.renderer.mesh(&points.data)?;
        mesh.set_transforms(false, true);

        let key = Self::create_key(position, drawing, stroke, cap_start, cap_end);
        let drawing_meshes = if let Some(meshes) = self.drawings.get_mut(&drawing.id) {
            meshes
        } else {
            self.drawings.insert(drawing.id, Vec::new());
            self.drawings.get_mut(&drawing.id).unwrap()
        };

        // If there are too many meshes for this drawing, remove the first half
        // of the vec to clear out the oldest meshes.
        if drawing_meshes.len() > Self::DRAWING_MAX_MESHES {
            drawing_meshes.drain(0..(Self::DRAWING_MAX_MESHES / 2));
        };

        drawing_meshes.push((key, mesh));
        Ok(())
    }

    fn get_drawing(
        &self,
        position: Rect,
        drawing: &scene::Drawing,
        stroke: f32,
        start: scene::Cap,
        end: scene::Cap,
    ) -> Option<&Mesh> {
        if let Some(meshes) = self.drawings.get(&drawing.id) {
            let key = Self::create_key(position, drawing, stroke, start, end);
            for (mesh_key, mesh) in meshes {
                if *mesh_key == key {
                    return Some(mesh);
                }
            }
        }
        None
    }

    fn update_grid_size(&mut self, grid_size: f32) {
        if self.grid_size != grid_size {
            self.grid_size = grid_size;
            self.drawings.clear();
        }
    }

    pub fn draw_drawing(
        &mut self,
        drawing: &scene::Drawing,
        stroke: f32,
        start: scene::Cap,
        end: scene::Cap,
        colour: scene::Colour,
        viewport: Rect,
        position: Rect,
        grid_size: f32,
    ) {
        self.update_grid_size(grid_size);
        if let Some(mesh) = self.get_drawing(position, drawing, stroke, start, end) {
            self.renderer
                .draw(mesh, colour, viewport, position * grid_size);
        } else if self
            .add_drawing(position, drawing, stroke, start, end)
            .is_ok()
        {
            self.draw_drawing(
                drawing, stroke, start, end, colour, viewport, position, grid_size,
            );
        }
    }
}
