use scene::Rect;

use super::{to_unit, webgl::LineRenderer, ViewInfo};

pub struct GridRenderer {
    line_renderer: LineRenderer,
    current_vp: Option<Rect>,
    current_grid_dims: Option<(u32, u32)>,
    current_grid_size: Option<f32>,
    current_line_count: Option<i32>,
}

impl GridRenderer {
    pub fn new(inner: LineRenderer) -> GridRenderer {
        GridRenderer {
            line_renderer: inner,
            current_vp: None,
            current_grid_dims: None,
            current_grid_size: None,
            current_line_count: None,
        }
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
