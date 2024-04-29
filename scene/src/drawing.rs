use serde_derive::{Deserialize, Serialize};

use super::{Id, Point, PointVector, Rect};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum DrawingMode {
    Cone,
    Freehand,
    Line,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum DrawingInner {
    Freehand(PointVector),
    Line(Point, Point),
}

impl DrawingInner {
    fn new(mode: DrawingMode) -> Self {
        match mode {
            DrawingMode::Cone | DrawingMode::Line => {
                DrawingInner::Line(Point::ORIGIN, Point::ORIGIN)
            }
            DrawingMode::Freehand => DrawingInner::Freehand(PointVector::new()),
        }
    }

    fn from(mode: DrawingMode, points: PointVector) -> Self {
        match mode {
            DrawingMode::Cone | DrawingMode::Line => DrawingInner::Line(
                points.nth(1).unwrap_or_default(),
                points.last().unwrap_or_default(),
            ),
            DrawingMode::Freehand => DrawingInner::Freehand(points),
        }
    }

    fn add(&mut self, point: Point) {
        match self {
            DrawingInner::Freehand(points) => {
                // Adds a new point to the drawing, if it isn't too close to the previous
                // point.
                const MINIMUM_DISTANCE: f32 = 0.1;

                if let Some(prev) = points.last() {
                    if prev.dist(point) < MINIMUM_DISTANCE {
                        return;
                    }
                }

                points.add(point);
            }
            DrawingInner::Line(start, end) => {
                if *start == Point::ORIGIN && *end == Point::ORIGIN {
                    *start = point;
                }
                *end = point;
            }
        }
    }

    fn line(&self) -> (Point, Point) {
        match self {
            DrawingInner::Freehand(points) => {
                let p = points.nth(1).unwrap_or(Point::ORIGIN);
                let q = points.last().unwrap_or(Point::ORIGIN);
                (p, q)
            }
            &DrawingInner::Line(p, q) => (p, q),
        }
    }

    fn end(&self) -> Option<Point> {
        match self {
            DrawingInner::Freehand(points) => points.last(),
            &DrawingInner::Line(_, end) => Some(end),
        }
    }

    /// Simplifies the drawing such that its top-left-most point is the
    /// origin, returning its from rect before the transformation.
    fn simplify(&mut self) -> Rect {
        let rect = self.rect();
        let delta = rect.top_left();
        if delta.non_zero() {
            match self {
                DrawingInner::Freehand(points) => {
                    points.translate(-delta);
                }
                DrawingInner::Line(start, end) => {
                    *start = *start - delta;
                    *end = *end - delta;
                }
            }
        }
        rect
    }

    fn length(&self) -> f32 {
        match self {
            DrawingInner::Freehand(points) => {
                let mut dist = 0.0;
                let mut prev = None;
                points.iter(|p| {
                    if let Some(q) = prev {
                        dist += p.dist(q);
                    }
                    prev = Some(p);
                });
                dist
            }
            &DrawingInner::Line(p, q) => p.dist(q),
        }
    }

    fn rect(&self) -> Rect {
        match self {
            DrawingInner::Freehand(points) => points.rect(),
            DrawingInner::Line(p, q) => Rect {
                x: p.x.min(q.x),
                y: p.y.min(q.y),
                w: (p.x - q.x).abs(),
                h: (p.y - q.y).abs(),
            },
        }
    }

    fn encode(self) -> Vec<u8> {
        let points = match self {
            DrawingInner::Freehand(points) => points,
            DrawingInner::Line(p, q) => {
                let mut points = PointVector::new();
                points.add(p);
                points.add(q);
                points
            }
        };

        points.data.iter().flat_map(|f| f.to_be_bytes()).collect()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Drawing {
    pub id: Id,
    pub mode: DrawingMode,
    inner: DrawingInner,
}

impl Drawing {
    pub fn new(id: Id, mode: DrawingMode) -> Self {
        Self {
            id,
            mode,
            inner: DrawingInner::new(mode),
        }
    }

    pub fn from(id: Id, mode: DrawingMode, points: PointVector) -> Self {
        Self {
            id,
            mode,
            inner: DrawingInner::from(mode, points),
        }
    }

    pub fn line(&self) -> (Point, Point) {
        self.inner.line()
    }

    pub fn last_point(&self) -> Option<Point> {
        self.inner.end()
    }

    pub fn add_point(&mut self, point: Point) {
        self.inner.add(point);
    }

    /// Simplifies the drawing such that its top-left-most point is the
    /// origin, returning its from rect before the transformation.
    pub fn simplify(&mut self) -> Rect {
        self.inner.simplify()
    }

    pub fn length(&self) -> f32 {
        self.inner.length()
    }

    pub fn rect(&self) -> Rect {
        self.inner.rect()
    }

    pub fn n_points(&self) -> u32 {
        match &self.inner {
            DrawingInner::Freehand(points) => points.n() as u32,
            DrawingInner::Line(_, _) => 2,
        }
    }

    pub fn points(&self) -> Option<&PointVector> {
        if let DrawingInner::Freehand(points) = &self.inner {
            Some(points)
        } else {
            None
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        self.inner.clone().encode()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_freehand_has_points() {
        let drawing = Drawing::new(1, DrawingMode::Freehand);
        assert!(drawing.points().is_some());
        drawing.points().unwrap();
    }
}
