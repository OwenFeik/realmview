use std::ops::{Add, Div, Mul, Neg, Sub};

use serde_derive::{Deserialize, Serialize};

use super::Rect;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ORIGIN: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    /// Given an angle, returns a Point with the cos and sin of the angle.
    pub fn trig(theta: f32) -> Point {
        Point::new(theta.cos(), theta.sin())
    }

    pub fn same(value: f32) -> Point {
        Point { x: value, y: value }
    }

    pub fn dist(&self, other: Self) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    pub fn angle(&self, other: Self) -> f32 {
        let triangle = other - *self;
        triangle.y.atan2(triangle.x)
    }

    // Return the rectangle formed by these two points.
    pub fn rect(&self, Point { x, y }: Point) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            w: x - self.x,
            h: y - self.y,
        }
    }

    pub fn non_zero(&self) -> bool {
        self.x != 0.0 || self.y != 0.0
    }

    #[must_use]
    pub fn round(&self) -> Self {
        Point {
            x: self.x.round(),
            y: self.y.round(),
        }
    }
}

impl Default for Point {
    fn default() -> Self {
        Self::ORIGIN
    }
}

impl Add for Point {
    type Output = Point;

    fn add(self, rhs: Point) -> Point {
        Point {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Mul<Point> for Point {
    type Output = Point;

    fn mul(self, rhs: Point) -> Self::Output {
        Point {
            x: rhs.x * self.x,
            y: rhs.y * self.y,
        }
    }
}

impl Mul<f32> for Point {
    type Output = Point;

    fn mul(self, rhs: f32) -> Self::Output {
        Point {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl Div<Point> for Point {
    type Output = Point;
    fn div(self, rhs: Point) -> Self::Output {
        Point {
            x: self.x / rhs.x,
            y: self.y / rhs.y,
        }
    }
}

impl Div<f32> for Point {
    type Output = Point;
    fn div(self, rhs: f32) -> Self::Output {
        Point {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}

impl Neg for Point {
    type Output = Point;

    fn neg(self) -> Self::Output {
        Point {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl Sub for Point {
    type Output = Point;

    fn sub(self, rhs: Point) -> Point {
        Point {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PointVector {
    pub data: Vec<f32>,
}

impl PointVector {
    pub fn new() -> Self {
        Self::from(Vec::new())
    }

    pub fn from(data: Vec<f32>) -> Self {
        Self { data }
    }

    pub fn sized(n: u32) -> Self {
        Self::from(Vec::with_capacity((n * 2) as usize))
    }

    pub fn origin() -> Self {
        Self::from(vec![0.0, 0.0])
    }

    pub fn n(&self) -> usize {
        self.data.len() / 2
    }

    pub fn keep_n(&mut self, n: usize) {
        self.data.truncate(n * 2)
    }

    pub fn nth(&self, i: usize) -> Option<Point> {
        if i >= 1 && i <= self.n() {
            Some(Point {
                x: self.data[2 * i - 2],
                y: self.data[2 * i - 1],
            })
        } else {
            None
        }
    }

    pub fn iter<F: FnMut(Point)>(&self, mut func: F) {
        for i in (0..self.data.len()).step_by(2) {
            func(Point {
                x: self.data[i],
                y: self.data[i + 1],
            });
        }
    }

    pub fn map<F: FnMut(Point) -> Point>(&mut self, mut func: F) {
        for i in (0..self.data.len()).step_by(2) {
            let Point { x, y } = func(Point {
                x: self.data[i],
                y: self.data[i + 1],
            });
            self.data[i] = x;
            self.data[i + 1] = y;
        }
    }

    pub fn last(&self) -> Option<Point> {
        self.nth(self.n())
    }

    pub fn add(&mut self, point: Point) {
        self.data.push(point.x);
        self.data.push(point.y);
    }

    pub fn add_point(&mut self, x: f32, y: f32) {
        self.data.push(x);
        self.data.push(y);
    }

    pub fn add_tri(&mut self, a: Point, b: Point, c: Point) {
        self.add(a);
        self.add(b);
        self.add(c);
    }

    pub fn rect(&self) -> Rect {
        let mut x_min = std::f32::MAX;
        let mut x_max = std::f32::MIN;
        let mut y_min = std::f32::MAX;
        let mut y_max = std::f32::MIN;

        self.iter(|Point { x, y }| {
            x_min = x_min.min(x);
            x_max = x_max.max(x);
            y_min = y_min.min(y);
            y_max = y_max.max(y);
        });

        Rect::new(x_min, y_min, x_max - x_min, y_max - y_min)
    }

    pub fn scale(&mut self, scale: f32) {
        self.map(|p| p * scale);
    }

    pub fn scale_asymmetric(&mut self, scale_x: f32, scale_y: f32) {
        let scale = Point::new(scale_x, scale_y);
        self.map(|p| p * scale);
    }

    pub fn translate(&mut self, delta: Point) {
        self.map(|p| p + delta);
    }
}

impl Default for PointVector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::{Point, PointVector};
    use crate::Rect;

    #[test]
    fn test_rect() {
        let mut pv = PointVector::new();

        //             / /\
        //    /\      /  |
        //  /   \    /   | Height: [1.0, -1.5] = 2.5
        // -     \  /    |
        //        \/    \/
        //  <----------> Width: [-0.5, 5.5] = 6.0

        pv.add_point(-0.5, 0.0);
        pv.add(Point::ORIGIN);
        pv.add_point(0.5, -0.5);
        pv.add_point(1.0, -1.0);
        pv.add_point(1.5, -0.5);
        pv.add_point(2.0, 0.0);
        pv.add_point(2.5, 0.5);
        pv.add_point(3.0, 1.0);
        pv.add_point(3.5, 0.5);
        pv.add_point(4.0, 0.0);
        pv.add_point(4.5, -0.5);
        pv.add_point(5.0, -1.0);
        pv.add_point(5.5, -1.5);

        assert_eq!(pv.rect(), Rect::new(-0.5, -1.5, 6.0, 2.5));
    }

    #[test]
    fn test_rect_negative() {
        let mut pv = PointVector::new();

        pv.add_point(-0.5, -0.5);
        pv.add_point(-1.0, -1.0);

        assert_eq!(pv.rect(), Rect::new(-1.0, -1.0, 0.5, 0.5));
    }
}
