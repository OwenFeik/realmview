use serde_derive::{Deserialize, Serialize};

use super::{Id, Point, PointVector, Rect};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Drawing {
    pub id: Id,
    pub points: PointVector,
    pub finished: bool,
}

impl Drawing {
    pub fn new(id: Id) -> Self {
        Self {
            id,
            ..Default::default()
        }
    }

    pub fn line(&self) -> (Point, Point) {
        let p = self.points.nth(1).unwrap_or(Point::ORIGIN);
        let q = self.points.last().unwrap_or(Point::ORIGIN);
        (p, q)
    }

    pub fn n_points(&self) -> u32 {
        self.points.n() as u32
    }

    pub fn keep_n_points(&mut self, n: u32) {
        self.points.keep_n(n as usize);
    }

    pub fn last_point(&self) -> Option<Point> {
        self.points.last()
    }

    // Adds a new point to the drawing, if it isn't too close to the previous
    // point.
    pub fn add_point(&mut self, point: Point) {
        const MINIMUM_DISTANCE: f32 = 0.1;

        if let Some(prev) = self.points.last() {
            if prev.dist(point) < MINIMUM_DISTANCE {
                return;
            }
        }

        self.points.add(point);
    }

    /// Simplifies the drawing such that it's top-left-most point is the
    /// origin, returning it's from rect before the transformation.
    pub fn simplify(&mut self) -> Rect {
        let rect = self.points.rect();
        let delta = rect.top_left();
        if delta.non_zero() {
            self.points.translate(-delta);
        }
        rect
    }

    fn translate(&mut self, offset: f32) {
        self.points.translate(Point::same(offset));
    }

    fn scale(&mut self, sx: f32, sy: f32) {
        self.points.map(|p| Point::new(p.x * sx, p.y * sy));
    }
}

impl Default for Drawing {
    fn default() -> Self {
        Self {
            id: 0,
            points: PointVector::from(vec![0.0, 0.0]),
            finished: false,
        }
    }
}
