use std::collections::HashMap;

use scene::Rect;

use super::webgl::{Mesh, SolidRenderer};
use crate::Res;

pub struct DrawingRenderer {
    grid_size: f32,
    drawings: HashMap<scene::Id, (u128, Mesh)>,
    renderer: SolidRenderer,
}

impl DrawingRenderer {
    pub fn new(inner: SolidRenderer) -> Self {
        Self {
            grid_size: 0.0,
            drawings: HashMap::new(),
            renderer: inner,
        }
    }

    fn create_key(
        drawing: &scene::Drawing,
        stroke: f32,
        cap_start: scene::Cap,
        cap_end: scene::Cap,
    ) -> u128 {
        // Key format is a u128 with the following structure:
        //
        // 32 bits for the rect width
        // 32 bits for the rect height
        // 32 bits for the stroke width
        // 28 bits counting the number of points in the drawing
        // 2 bits for the starting cap
        // 2 bits for the ending cap
        // 1 bit for whether the drawing is finished
        //
        // Like so:
        // 000000000000000000000000000WIDTH00000000000000000000000000HEIGHT
        // 00000000000000000000000000STROKE000000000000000N_POINTSSTRT0ENDF
        //
        // Is this grotesquely overcomplicated? Yes.
        let mut key = 0u128;

        let rect = drawing.rect();

        // First 100 bits are the literal bits of the three floats.
        let mut keyf32 = |v: f32| {
            key |= v.to_bits() as u128;
            key <<= 32;
        };
        keyf32(rect.w); // 32
        keyf32(rect.h); // 64
        keyf32(stroke); // 96

        // Last 28 bits. We've already shifted the first 100 across by 100.
        let mut low = 0u32;
        low |= drawing.n_points();
        low <<= 28; // Assume n_points is smaller than 28 bits.
        low |= cap_start as u32; // allow 2 bits
        low <<= 2;
        low |= cap_end as u32; // 2 bits
        key |= low as u128;

        key
    }

    fn add_drawing(
        &mut self,
        id: scene::Id,
        rect: scene::Rect,
        drawing: &scene::Drawing,
        stroke: f32,
        cap_start: scene::Cap,
        cap_end: scene::Cap,
    ) -> Res<()> {
        let points = match drawing.mode {
            scene::DrawingMode::Freehand => {
                let points_rect = drawing.rect();
                super::shapes::freehand(
                    drawing.points().unwrap(),
                    stroke,
                    cap_start,
                    cap_end,
                    (
                        self.grid_size * rect.w / points_rect.w,
                        self.grid_size * rect.h / points_rect.h,
                    ),
                )
            }
            scene::DrawingMode::Line => {
                super::shapes::line(drawing.line(), stroke, cap_start, cap_end, self.grid_size)
            }
            scene::DrawingMode::Cone => super::shapes::cone(drawing.line(), self.grid_size),
        };

        let mut mesh = self.renderer.mesh(&points)?;
        mesh.set_transforms(false, true);
        self.drawings.insert(
            id,
            (Self::create_key(drawing, stroke, cap_start, cap_end), mesh),
        );
        Ok(())
    }

    fn get_drawing(
        &self,
        id: scene::Id,
        drawing: &scene::Drawing,
        stroke: f32,
        start: scene::Cap,
        end: scene::Cap,
    ) -> Option<&Mesh> {
        if let Some((key, mesh)) = self.drawings.get(&id) {
            if Self::create_key(drawing, stroke, start, end) == *key {
                return Some(mesh);
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
        let id = drawing.id;
        if let Some(mesh) = self.get_drawing(id, drawing, stroke, start, end) {
            self.renderer
                .draw(mesh, colour, viewport, position * grid_size);
        } else if self
            .add_drawing(id, position, drawing, stroke, start, end)
            .is_ok()
        {
            self.draw_drawing(
                drawing, stroke, start, end, colour, viewport, position, grid_size,
            );
        }
    }
}
