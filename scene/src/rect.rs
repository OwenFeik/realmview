use std::ops::{Add, Div, Mul, MulAssign, Sub};

use serde_derive::{Deserialize, Serialize};

use super::ScenePoint;

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

    pub fn from_point(point: ScenePoint, w: f32, h: f32) -> Rect {
        Rect {
            x: point.x,
            y: point.y,
            w,
            h,
        }
    }

    pub fn scaled_from(from: Rect, factor: f32) -> Rect {
        let mut rect = from;
        rect *= factor;
        rect
    }

    pub fn set_dimension(&mut self, dimension: Dimension, value: f32) {
        match dimension {
            Dimension::X => {
                self.x = value;
            }
            Dimension::Y => {
                self.y = value;
            }
            Dimension::W => {
                self.w = value;
            }
            Dimension::H => {
                self.h = value;
            }
        };
    }

    pub fn translate(&mut self, ScenePoint { x: dx, y: dy }: ScenePoint) {
        self.x += dx;
        self.y += dy;
    }

    #[must_use]
    fn positive_dimensions(&self) -> Self {
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

    pub fn contains_point(&self, point: ScenePoint) -> bool {
        // A negative dimension causes a texture to be flipped. As this is a useful behaviour, negative dimensions on
        // Rects are supported. To that end a different treatment is required for checking if a point is contained.
        // Hence the special cases for negative width and height.

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

    pub fn contains_rect(&self, rect: Rect) -> bool {
        let a = self.positive_dimensions();
        let b = rect.positive_dimensions();

        b.x >= a.x && b.x + b.w <= a.x + a.w && b.y >= a.y && b.y + b.h <= a.y + a.h
    }

    pub fn top_left(&self) -> ScenePoint {
        ScenePoint {
            x: self.x,
            y: self.y,
        }
    }

    pub fn delta(&self, other: Rect) -> f32 {
        let rect = other - *self;
        (rect.x + rect.y + rect.w + rect.h).abs()
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
