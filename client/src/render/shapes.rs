use std::f32::consts::{PI, TAU};

use crate::scene::{Point, PointVector, Rect};

const RADIUS: f32 = 0.5;
const CIRCLE_EDGES: u32 = 64;

/// Adds points for a regular polygon with n edges.
///
/// Note the resultant shape will be oriented with the first vertex at the
/// top center of the tile, i.e. a 4gon is a diamond and not a square.
fn add_ngon(dst: &mut PointVector, n: u32, c: Point, r: f32) {
    let dt = TAU / n as f32;

    let mut prev = None;
    for i in 0..=n {
        let theta = i as f32 * dt;
        let q = c + Point::trig(theta) * r;
        if let Some(p) = prev {
            dst.add_tri(p, q, c);
        }
        prev = Some(q);
    }
}

/// Adds points for a regular polygon outline with n edges.
///
/// Note the resultant shape will be oriented with the first vertex at the
/// top center of the tile, i.e. a 4gon is a diamond and not a square.
fn add_ngon_outline(dst: &mut PointVector, n: u32, rect: Rect) {
    let dt = TAU / n as f32;

    let c = rect.centre();
    let r = Point::new(rect.w / 2.0, rect.h / 2.0);

    for i in 0..n {
        let theta = i as f32 * dt;
        let q = c + Point::trig(theta) * r;
        dst.add(q);
    }
}

/// Returns points for a hollow regular polygon with n edges, a stroke width
/// given by the top left corner of rect and dimension of the rect.
fn add_hollow_ngon(dst: &mut PointVector, n: u32, rect: Rect) {
    let c = Point::new(rect.w / 2.0, rect.h / 2.0);
    let ra = c;
    let rb = ra - rect.top_left();
    let dt = TAU / n as f32;

    let mut prev_a = None;
    let mut prev_b = None;
    for i in 0..=n {
        let theta = i as f32 * dt;
        let delta = Point::trig(theta);
        let a = c + delta * ra;
        let b = c + delta * rb;

        if let (Some(pa), Some(pb)) = (prev_a, prev_b) {
            dst.add_tri(pa, pb, a);
            dst.add_tri(a, b, pb);
        }

        prev_a = Some(a);
        prev_b = Some(b);
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
        let a = c + Point::trig(theta) * r;
        if let Some(b) = prev {
            dst.add_tri(a, b, c);
        }
        prev = Some(a);
    }
}

/// Draws a line cap at a given point, in a direction, at a size given by
/// stroke.
fn add_cap(dst: &mut PointVector, cap: scene::Cap, at: Point, direction: f32, stroke: f32) {
    const ARROWHEAD_MULTIPLIER: f32 = 4.0;

    let theta = direction;
    match cap {
        scene::Cap::Arrow => {
            let r = ARROWHEAD_MULTIPLIER * stroke / 2.0;
            let left = theta - PI / 2.0;
            let right = theta + PI / 2.0;

            dst.add_tri(
                at + Point::trig(left) * r,
                at + Point::trig(right) * r,
                at + Point::trig(theta) * r * 2.0,
            );
        }
        scene::Cap::Round => add_semicircle(dst, at, stroke / 2.0, direction - PI / 2.0),
        scene::Cap::None => {}
    }
}

/// Given a series of (x, y) coordinates, points, and a line width, produces a
/// series of triangles (x1, y1, x2, y2, x3, y3) to render the drawing defined
/// by those points. Assumes the input array is in scene units and produces
/// points pre-scaled to [-1, 1] for drawing.
fn add_line(
    dst: &mut PointVector,
    points: &PointVector,
    stroke: f32,
    cap_start: scene::Cap,
    cap_end: scene::Cap,
) {
    const CIRCLE_EDGES: u32 = 32;

    let n = points.n();

    if n < 2 {
        return;
    }

    let r = stroke / 2.0;

    // Previous line endponts, used to close up gaps at corners
    let mut prev_c: Option<Point> = None;
    let mut prev_d: Option<Point> = None;

    let last = n - 1;
    for i in 1..n {
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
        let above = Point::trig(pos) * r;
        let below = Point::trig(neg) * r;

        // Calculate points
        let a = p + above;
        let b = p + below;
        let c = q + below;
        let d = q + above;

        // Draw line segment
        dst.add_tri(a, b, c);
        dst.add_tri(a, c, d);

        // Draw caps for first and last line segment
        if i == 1 {
            add_cap(dst, cap_start, p, theta - PI, stroke);
        }
        if i == last {
            add_cap(dst, cap_end, q, theta, stroke);
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
}

fn hollow_rectangle(rect: Rect) -> Vec<f32> {
    let mut dst = PointVector::new();

    let tl = Point::ORIGIN;
    let tls = tl + rect.top_left();
    let tr = Point::new(rect.w, 0.0);
    let trs = tr + Point::new(-rect.x, rect.y);
    let bl = Point::new(0.0, rect.h);
    let bls = bl + Point::new(rect.x, -rect.y);
    let br = Point::new(rect.w, rect.h);
    let brs = br - rect.top_left();

    dst.add_tri(tl, tr, tls);
    dst.add_tri(tls, tr, trs);
    dst.add_tri(trs, br, tr);
    dst.add_tri(trs, brs, br);
    dst.add_tri(brs, br, bl);
    dst.add_tri(bls, brs, bl);
    dst.add_tri(bls, tls, bl);
    dst.add_tri(bl, tls, tl);

    dst.data
}

fn shape_edges(shape: scene::Shape) -> u32 {
    match shape {
        scene::Shape::Ellipse => CIRCLE_EDGES,
        scene::Shape::Hexagon => 6,
        scene::Shape::Rectangle => 4,
        scene::Shape::Triangle => 3,
    }
}

fn ngon_outline(n: u32, rect: Rect) -> Vec<f32> {
    let mut coords = PointVector::sized(n);
    add_ngon_outline(&mut coords, n, rect);
    coords.data
}

/// Return a polygon with n edges which fits in a 1*1 square, centred at
/// (0.5, 0.5).
pub fn ngon(n: u32) -> Vec<f32> {
    let mut coords = PointVector::sized(n * 3);
    add_ngon(&mut coords, n, Point::same(RADIUS), RADIUS);
    coords.data
}

pub fn hollow_ngon(n: u32, rect: Rect) -> Vec<f32> {
    let mut coords = PointVector::sized(n * 2 * 3);
    add_hollow_ngon(&mut coords, n, rect);
    coords.data
}

pub fn circle() -> Vec<f32> {
    ngon(CIRCLE_EDGES)
}

pub fn rectangle() -> Vec<f32> {
    const RECTANGLE: &[f32] = &[0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    RECTANGLE.to_owned()
}

pub fn shape(shape: scene::Shape) -> Vec<f32> {
    match shape {
        scene::Shape::Rectangle => rectangle(),
        _ => ngon(shape_edges(shape)),
    }
}

pub fn hollow_shape(shape: scene::Shape, rect: Rect) -> Vec<f32> {
    match shape {
        scene::Shape::Rectangle => hollow_rectangle(rect),
        _ => hollow_ngon(shape_edges(shape), rect),
    }
}

pub fn outline_shape(shape: scene::Shape, rect: Rect) -> Vec<f32> {
    match shape {
        scene::Shape::Rectangle => {
            let Rect { x, y, w, h } = rect;
            vec![x, y, x + w, y, x + w, y + h, x, y + h]
        }
        _ => ngon_outline(shape_edges(shape), rect),
    }
}

pub fn line(
    (p, q): (Point, Point),
    stroke: f32,
    cap_start: scene::Cap,
    cap_end: scene::Cap,
) -> PointVector {
    let mut coords = PointVector::new();
    add_line(
        &mut coords,
        &PointVector::from(vec![p.x, p.y, q.x, q.y]),
        stroke,
        cap_start,
        cap_end,
    );
    coords
}

pub fn freehand(
    points: &PointVector,
    stroke: f32,
    cap_start: scene::Cap,
    cap_end: scene::Cap,
) -> PointVector {
    let mut coords = PointVector::new();
    add_line(&mut coords, points, stroke, cap_start, cap_end);
    coords
}

pub fn cone((p, q): (Point, Point)) -> PointVector {
    let h = p.dist(q);
    let theta = p.angle(q);

    let left = q + Point::trig(theta - PI / 2.0) * h * 0.5;
    let right = q + Point::trig(theta + PI / 2.0) * h * 0.5;

    let mut coords = PointVector::new();
    coords.add_tri(left, right, p);
    coords
}

#[cfg(test)]
mod test {
    use scene::{float_eq, Cap};

    use super::*;

    #[test]
    fn test_straight_line_start_point() {
        let p = Point::same(-1.0);
        let q = Point::same(1.0);
        let stroke = 1.0;
        let points = line((p, q), stroke, Cap::None, Cap::None);

        // Should have two triangles, splitting the diagonal of the line.
        assert_eq!(points.data.len(), 6 * 2);

        // Should have 3 points at each end to form the two triangles.
        let mut start = 0;
        let mut end = 0;
        points.iter(|point| {
            if float_eq(point.dist(p), stroke / 2.0) {
                start += 1
            } else if float_eq(point.dist(q), stroke / 2.0) {
                end += 1;
            }
        });
        assert_eq!(start, 3);
        assert_eq!(end, 3);
    }
}
