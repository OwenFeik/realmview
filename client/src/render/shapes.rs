use std::f32::consts::PI;

use scene::{Point, PointVector};

const CIRCLE_EDGES: u32 = 32;
const RECTANGLE: &[f32] = &[0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];

// Returns points for a regular polygon with n edges.
//
// Note the resultant shape will be oriented with the first vertex at the
// top center of the tile, i.e. a 4gon is a diamond and not a square.
fn add_ngon(dst: &mut PointVector, n: u32, c: Point, r: f32) {
    let dt = 2.0 * PI / n as f32;

    let mut prev = None;
    for i in 0..=n {
        let theta = i as f32 * dt;
        let q = Point::new(c.x + r * theta.cos(), c.y + r * theta.sin());
        if let Some(p) = prev {
            dst.add_tri(p, q, c);
        }
        prev = Some(q);
    }
}

/// Adds points for a semicircle to dst. The centre point of the semicircle is
/// c, the radius is r. The semicircle arc is pi radians from start.
fn add_semicircle(dst: &mut PointVector, c: Point, r: f32, start: f32) {
    let n = CIRCLE_EDGES / 2;
    let mut prev = None;
    let dt = PI / n as f32;
    for i in 0..=n {
        let theta = start + dt * i as f32;
        let a = c + Point {
            x: r * theta.cos(),
            y: r * theta.sin(),
        };
        if let Some(b) = prev {
            dst.add_tri(a, b, c);
        }
        prev = Some(a);
    }
}

/// Draws a line cap at a given point, in a direction, at a size given by
/// stroke.
fn add_cap(dst: &mut PointVector, cap: scene::SpriteCap, at: Point, direction: f32, stroke: f32) {
    const ARROWHEAD_MULTIPLIER: f32 = 4.0;

    let t = direction;
    match cap {
        scene::SpriteCap::Arrow => {
            let r = ARROWHEAD_MULTIPLIER * stroke / 2.0;
            let left = t + PI / 2.0;
            let right = t + PI / 2.0;
            dst.add_tri(
                at + Point::new(left.cos(), left.sin()) * r,
                at + Point::new(right.cos(), right.sin()) * r,
                at + Point::new(t.cos(), t.sin()) * r * 2.0,
            );
        }
        scene::SpriteCap::Round => add_semicircle(dst, at, stroke / 2.0, direction + PI / 2.0),
        _ => {}
    }
}

pub fn ngon(n: u32) -> Vec<f32> {
    const R: f32 = 0.5;
    let mut coords = PointVector::sized(n * 3);
    add_ngon(&mut coords, n, Point::new(R, R), R);
    coords.data
}

pub fn circle() -> Vec<f32> {
    ngon(CIRCLE_EDGES)
}

pub fn rectangle() -> &'static [f32] {
    RECTANGLE
}

/// Given a series of (x, y) coordinates, points, and a line width, produces a
/// series of triangles (x1, y1, x2, y2, x3, y3) to render the drawing defined
/// by those points. Assumes the input array is in scene units and produces
/// points pre-scaled to [-1, 1] for drawing.
pub fn drawing(
    points: &PointVector,
    stroke: f32,
    cap_start: scene::SpriteCap,
    cap_end: scene::SpriteCap,
) -> Vec<f32> {
    const CIRCLE_EDGES: u32 = 32;

    let mut dst = PointVector::new();

    // Previous line endponts, used to close up gaps at corners
    let mut prev_c: Option<Point> = None;
    let mut prev_d: Option<Point> = None;

    let last = points.n();
    for i in 1..points.n() {
        // Rectangular line segment from p to q
        // Uses four points (a, b, c, d) around the two points to draw the
        // segment, like so:
        //
        //        (p) _ (a)
        //        _,o^ \
        //    (b) \  \  \
        //         \  \  \
        //          \  \  \ (d)
        //           \_,o~^
        //        (c)   (q)
        //

        // Safe to unwrap as we've already checked how many points there are
        let p = points.nth(i).unwrap();
        let q = points.nth(i + 1).unwrap();

        // Angle between points
        let theta = p.angle(q);

        // Normals above and below the line
        let pos = theta + PI / 2.0;
        let neg = theta - PI / 2.0;

        // Position changes to generate corner points
        let above = Point::new(stroke * pos.cos(), stroke * pos.sin());
        let below = Point::new(stroke * neg.cos(), stroke * neg.sin());

        // Calculate points
        let a = p + above;
        let b = p + below;
        let c = q + above;
        let d = q + below;

        // Draw line segment
        dst.add_tri(a, b, c);
        dst.add_tri(a, d, c);

        // Draw caps for first and last line segment
        if i == 1 {
            add_cap(&mut dst, cap_start, p, theta + PI, stroke);
        } else if i == last {
            add_cap(&mut dst, cap_end, q, theta, stroke);
        }

        // Draw triangles over on the corner to close up the gap
        if let (Some(pc), Some(pd)) = (prev_c, prev_d) {
            dst.add_tri(a, b, pc);
            dst.add_tri(a, b, pd);
        }

        // Store c and d for the next gap
        prev_c = Some(c);
        prev_d = Some(d);
    }

    dst.data
}
