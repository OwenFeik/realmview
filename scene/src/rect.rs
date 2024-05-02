use std::ops::{Add, Div, Mul, MulAssign, Sub};

use serde_derive::{Deserialize, Serialize};

use super::Point;

#[derive(Clone, Copy)]
pub enum Dimension {
    X,
    Y,
    W,
    H,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    pub fn at(point: Point, w: f32, h: f32) -> Rect {
        Rect {
            x: point.x,
            y: point.y,
            w,
            h,
        }
    }

    /// Square around point.
    pub fn around(point: Point, r: f32) -> Rect {
        Rect {
            x: point.x - r,
            y: point.y - r,
            w: 2.0 * r,
            h: 2.0 * r,
        }
    }

    pub fn zeroed() -> Self {
        Rect {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        }
    }

    /// Whether the rect is aligned to a full, half, or quarter tile grid cell.
    pub fn is_aligned(&self) -> bool {
        ((self.x % determine_unit_size(self.w)).abs() <= f32::EPSILON)
            && ((self.y % determine_unit_size(self.h)).abs() <= f32::EPSILON)
    }

    pub fn scaled_from(from: Rect, factor: f32) -> Rect {
        let mut rect = from;
        rect *= factor;
        rect
    }

    pub fn scaled(self, factor: f32) -> Rect {
        self * factor
    }

    pub fn match_aspect(mut self, other: Rect) -> Rect {
        let other = other.positive_dimensions();
        self.w = self.h * (other.w / other.h);
        self
    }

    #[must_use]
    pub fn dimension(&self, dimension: Dimension, value: f32) -> Rect {
        let mut rect = *self;
        match dimension {
            Dimension::X => {
                rect.x = value;
            }
            Dimension::Y => {
                rect.y = value;
            }
            Dimension::W => {
                rect.w = value;
            }
            Dimension::H => {
                rect.h = value;
            }
        };
        rect
    }

    #[must_use]
    pub fn translate(&self, Point { x: dx, y: dy }: Point) -> Self {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }

    pub fn translate_in_place(&mut self, by: Point) {
        *self = self.translate(by);
    }

    #[must_use]
    pub fn positive_dimensions(&self) -> Self {
        let mut new = *self;

        if self.w < 0.0 {
            new.x = self.x + self.w;
            new.w = self.w.abs();
        }

        if self.h < 0.0 {
            new.y = self.y + self.h;
            new.h = self.h.abs();
        }

        new
    }

    pub fn round(&mut self) {
        self.x = self.x.round();
        self.y = self.y.round();
        self.w = self.w.round();
        self.h = self.h.round();

        if self.w >= 0.0 && self.w < 1.0 {
            self.w = 1.0;
        } else if self.w <= 0.0 && self.w > -1.0 {
            self.w = -1.0;
        }

        if self.h >= 0.0 && self.h < 1.0 {
            self.h = 1.0;
        } else if self.h <= 0.0 && self.h > -1.0 {
            self.h = -1.0;
        }
    }

    pub fn contains_point(&self, point: Point) -> bool {
        // A negative dimension causes a texture to be flipped. As this is a
        // useful behaviour, negative dimensions on Rects are supported. To
        // that end a different treatment is required for checking if a point
        // is contained. Hence the special cases for negative width and height.

        let in_x = {
            if self.w < 0.0 {
                self.x + self.w <= point.x && point.x <= self.x
            } else {
                self.x <= point.x && point.x <= self.x + self.w
            }
        };

        let in_y = {
            if self.h < 0.0 {
                self.y + self.h <= point.y && point.y <= self.y
            } else {
                self.y <= point.y && point.y <= self.y + self.h
            }
        };

        in_x && in_y
    }

    pub fn containing(self, point: Point) -> Self {
        let mut rect = self.positive_dimensions();
        if point.x < rect.x {
            let dx = rect.x - point.x;
            rect.x = point.x;
            rect.w += dx;
        } else if point.x > rect.x + rect.w {
            rect.w = point.x - rect.x;
        }

        if point.y < rect.y {
            let dy = rect.y - point.y;
            rect.y = point.y;
            rect.h += dy;
        } else if point.y > rect.y + rect.h {
            rect.h = point.y - rect.y;
        }

        rect
    }

    pub fn dist_to_point(&self, point: Point) -> f32 {
        if self.w < 0.0 || self.h < 0.0 {
            self.positive_dimensions().dist_to_point(point)
        } else {
            let dx = (self.x - point.x).max(point.x - (self.x + self.w)).max(0.0);
            let dy = (self.y - point.y).max(point.y - (self.y + self.h)).max(0.0);
            (dx.powi(2) + dy.powi(2)).sqrt()
        }
    }

    pub fn contains_rect(&self, rect: Rect) -> bool {
        let a = self.positive_dimensions();
        let b = rect.positive_dimensions();

        b.x >= a.x && b.x + b.w <= a.x + a.w && b.y >= a.y && b.y + b.h <= a.y + a.h
    }

    pub fn centre(&self) -> Point {
        Point {
            x: self.x + self.w / 2.0,
            y: self.y + self.h / 2.0,
        }
    }

    pub fn top_left(&self) -> Point {
        Point {
            x: self.x,
            y: self.y,
        }
    }

    pub fn dimensions(&self) -> Point {
        Point {
            x: self.w,
            y: self.h,
        }
    }

    pub fn delta(&self, other: Rect) -> f32 {
        let rect = other - *self;
        rect.x.abs() + rect.y.abs() + rect.w.abs() + rect.h.abs()
    }

    #[must_use]
    pub fn moved_to(&self, point: Point) -> Self {
        Rect {
            x: point.x,
            y: point.y,
            w: self.w,
            h: self.h,
        }
    }

    #[must_use]
    pub fn sized_as(&self, w: f32, h: f32) -> Self {
        Rect::new(self.x, self.y, w, h)
    }
}

impl Add for Rect {
    type Output = Rect;

    fn add(self, rhs: Rect) -> Rect {
        Rect {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            w: self.w + rhs.w,
            h: self.h + rhs.h,
        }
    }
}

impl Sub for Rect {
    type Output = Rect;

    fn sub(self, rhs: Rect) -> Rect {
        Rect {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            w: self.w - rhs.w,
            h: self.h - rhs.h,
        }
    }
}

impl Mul<f32> for Rect {
    type Output = Rect;

    fn mul(self, rhs: f32) -> Rect {
        Rect {
            x: self.x * rhs,
            y: self.y * rhs,
            w: self.w * rhs,
            h: self.h * rhs,
        }
    }
}

impl MulAssign<f32> for Rect {
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
        self.w *= rhs;
        self.h *= rhs;
    }
}

impl Div<f32> for Rect {
    type Output = Rect;

    fn div(self, rhs: f32) -> Rect {
        Rect {
            x: self.x / rhs,
            y: self.y / rhs,
            w: self.w / rhs,
            h: self.h / rhs,
        }
    }
}

impl Div<Rect> for Rect {
    type Output = Rect;

    fn div(self, rhs: Rect) -> Rect {
        Rect {
            x: self.x / rhs.x,
            y: self.y / rhs.y,
            w: self.w / rhs.w,
            h: self.h / rhs.h,
        }
    }
}

pub fn float_eq(a: f32, b: f32) -> bool {
    (a - b).abs() <= f32::EPSILON
}

pub fn determine_unit_size(d: f32) -> f32 {
    if d.abs() < 0.5 {
        0.25
    } else if d.abs() < 1.0 {
        0.5
    } else {
        1.0
    }
}

#[cfg(test)]
mod test {
    use super::Rect;
    use crate::{rect::float_eq, Point};

    #[test]
    fn test_delta() {
        let origin = Rect::new(0.0, 0.0, 0.0, 0.0);
        let ones = Rect::new(1.0, 1.0, 1.0, 1.0);
        let halfneg = Rect::new(-1.0, -1.0, 1.0, 1.0);
        assert_eq!(origin.delta(ones), 4.0);
        assert_eq!(origin.delta(halfneg), 4.0);
        assert_eq!(ones.delta(halfneg), 4.0);
    }

    #[test]
    fn test_contains_point() {
        let rect = Rect::new(-1.0, -1.0, 2.0, 2.0);
        assert!(rect.contains_point(Point::same(-1.0)));
        assert!(rect.contains_point(Point::same(-0.5)));
        assert!(rect.contains_point(Point::ORIGIN));
        assert!(rect.contains_point(Point::same(0.5)));
        assert!(rect.contains_point(Point::same(1.0)));

        assert!(!rect.contains_point(Point::same(-1.01)));
        assert!(!rect.contains_point(Point::same(1.01)));

        assert!(rect.contains_point(Point { x: -1.0, y: 0.0 }));
        assert!(rect.contains_point(Point { x: 1.0, y: 0.0 }));
        assert!(rect.contains_point(Point { x: 0.0, y: -1.0 }));
        assert!(rect.contains_point(Point { x: 0.0, y: 1.0 }));

        assert!(!rect.contains_point(Point { x: -1.01, y: 0.0 }));
        assert!(!rect.contains_point(Point { x: 1.01, y: 0.0 }));
        assert!(!rect.contains_point(Point { x: 0.0, y: -1.01 }));
        assert!(!rect.contains_point(Point { x: 0.0, y: 1.01 }));
    }

    #[test]
    fn test_dist_to_point() {
        let rect = Rect::new(1.0, 1.0, 2.5, 2.5);

        assert!(float_eq(rect.dist_to_point(Point::same(3.5)), 0.0));
        assert!(float_eq(rect.dist_to_point(Point::same(1.0)), 0.0));
        assert!(float_eq(rect.dist_to_point(Point::same(2.0)), 0.0));
        assert!(float_eq(rect.dist_to_point(Point::same(2.5)), 0.0));

        assert!(float_eq(rect.dist_to_point(Point { x: 3.6, y: 3.5 }), 0.1));
        assert!(float_eq(rect.dist_to_point(Point { x: 0.9, y: 3.5 }), 0.1));
    }

    #[test]
    fn test_containing() {
        let rect = Rect::new(0., 0., 2., 2.);

        let points = &[
            Point::same(-1.),
            Point::new(-1., -5.),
            Point::same(3.),
            Point::same(0.5),
            Point::new(2.5, 0.5),
            Point::new(2.5, -0.5),
            Point::new(123.456, -12.34),
        ];

        for &point in points {
            assert!(rect.containing(point).contains_point(point));
        }
    }
}
