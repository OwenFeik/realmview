use std::collections::HashMap;

use scene::{Point, Rect};

use super::webgl::{Mesh, SolidRenderer};
use crate::Res;

/// Key properties of a mesh for a given sprite-drawing pair. A mesh may be
/// shared between frames or between sprites if all fields in this struct
/// match.
#[derive(PartialEq)]
struct MeshProps {
    /// Width of the sprite's rect.
    w: f32,

    /// Height of the sprite's rect.
    h: f32,

    /// Last point in the drawing.
    last: Point,

    /// Stroke width of the sprite.
    stroke: f32,

    /// Number of points in the drawing.
    n: u32,

    /// Start cap of the sprite.
    cap_start: scene::Cap,

    /// End cap of the sprite.
    cap_end: scene::Cap,
}

impl MeshProps {
    fn new(
        position: Rect,
        drawing: &scene::Drawing,
        stroke: f32,
        cap_start: scene::Cap,
        cap_end: scene::Cap,
    ) -> Self {
        Self {
            w: position.w,
            h: position.h,
            last: drawing.last_point().unwrap_or(Point::ORIGIN),
            stroke,
            n: drawing.n_points(),
            cap_start,
            cap_end,
        }
    }
}

pub struct DrawingRenderer {
    grid_size: f32,
    drawings: HashMap<i64, Vec<(MeshProps, Mesh)>>, //  { drawing_id: [(key, mesh)] }
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

    fn drawing_line(position: Rect, drawing: &scene::Drawing) -> (Point, Point) {
        let rect = drawing.rect();
        let origin = rect.top_left();
        let scale_x = position.w / rect.w;
        let scale_y = position.h / rect.h;
        let (p, q) = drawing.line();
        (
            Point {
                x: (p.x - origin.x) * scale_x,
                y: (p.y - origin.y) * scale_y,
            },
            Point {
                x: (q.x - origin.x) * scale_x,
                y: (q.y - origin.y) * scale_y,
            },
        )
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
            scene::DrawingMode::Line => super::shapes::line(
                Self::drawing_line(position, drawing),
                stroke,
                cap_start,
                cap_end,
            ),
            scene::DrawingMode::Cone => super::shapes::cone(Self::drawing_line(position, drawing)),
        };

        points.scale(self.grid_size);
        let mut mesh = self.renderer.mesh(&points.data)?;
        mesh.set_transforms(false, true);

        let key = MeshProps::new(position, drawing, stroke, cap_start, cap_end);
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
            let key = MeshProps::new(position, drawing, stroke, start, end);
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
