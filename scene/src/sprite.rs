use serde_derive::{Deserialize, Serialize};

use crate::Dimension;

use super::{comms::SceneEvent, Id, Rect, ScenePoint};

pub type Colour = [f32; 4];

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum Shape {
    Ellipse,
    Hexagon,
    Rectangle,
    Triangle,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum Visual {
    Texture {
        shape: Shape,
        id: Id,
    },
    Solid {
        shape: Shape,
        stroke: f32,
        colour: Colour,
    },
    Drawing {
        stroke: f32,
        colour: Colour,
        points: Vec<f32>,
    },
}

impl Visual {
    pub fn colour(&self) -> Option<Colour> {
        match self {
            Self::Solid { colour, .. } | Self::Drawing { colour, .. } => Some(*colour),
            _ => None,
        }
    }

    pub fn texture(&self) -> Option<Id> {
        match self {
            Self::Texture { id, shape: _ } => Some(*id),
            _ => None,
        }
    }

    pub fn shape(&self) -> Option<Shape> {
        match self {
            Self::Solid { shape, .. } | Self::Texture { id: _, shape } => Some(*shape),
            _ => None,
        }
    }

    pub fn points(&self) -> Option<Vec<f32>> {
        if let Self::Drawing { points, .. } = self {
            Some(points.clone())
        } else {
            None
        }
    }

    pub fn stroke(&self) -> Option<f32> {
        match self {
            Self::Solid {
                colour: _,
                shape: _,
                stroke,
            }
            | Self::Drawing {
                colour: _,
                points: _,
                stroke,
            } => Some(*stroke),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Sprite {
    pub id: Id,
    pub rect: Rect,
    pub z: i32,
    pub visual: Visual,
}

impl Sprite {
    // Width of lines for sprites which are drawings
    pub const DEFAULT_STROKE: f32 = 0.2;
    pub const SOLID_STROKE: f32 = 0.0;

    // Minimum size of a sprite dimension; too small and sprites can be lost.
    const MIN_SIZE: f32 = 0.25;
    const DEFAULT_VISUAL: Visual = Visual::Solid {
        colour: [1.0, 0.0, 1.0, 1.0],
        shape: Shape::Rectangle,
        stroke: Sprite::SOLID_STROKE,
    };

    pub fn new(id: Id, visual: Option<Visual>) -> Sprite {
        Sprite {
            rect: Rect::new(0.0, 0.0, 1.0, 1.0),
            z: 1,
            visual: visual.unwrap_or(Sprite::DEFAULT_VISUAL),
            id,
        }
    }

    pub fn set_pos(&mut self, ScenePoint { x, y }: ScenePoint) -> SceneEvent {
        let from = self.rect;
        self.rect.x = x;
        self.rect.y = y;

        SceneEvent::SpriteMove(self.id, from, self.rect)
    }

    pub fn set_dimension(&mut self, dimension: Dimension, value: f32) -> SceneEvent {
        let from = self.rect;
        self.rect.set_dimension(dimension, value);
        SceneEvent::SpriteMove(self.id, from, self.rect)
    }

    pub fn set_rect(&mut self, rect: Rect) -> SceneEvent {
        let from = self.rect;
        self.rect = rect;
        SceneEvent::SpriteMove(self.id, from, self.rect)
    }

    fn set_size(&mut self, w: f32, h: f32) {
        self.rect.w = w;
        self.rect.h = h;
    }

    pub fn set_visual(&mut self, mut new: Visual) -> SceneEvent {
        std::mem::swap(&mut new, &mut self.visual);
        SceneEvent::SpriteVisual(self.id, new, self.visual.clone())
    }

    pub fn snap_pos(&mut self) -> SceneEvent {
        let old = self.rect;
        self.rect.x = round_to_nearest(old.x, determine_unit_size(old.w));
        self.rect.y = round_to_nearest(old.y, determine_unit_size(old.h));
        SceneEvent::SpriteMove(self.id, old, self.rect)
    }

    pub fn snap_size(&mut self) -> SceneEvent {
        let old = self.rect;
        self.rect.w = round_dimension(old.w);
        self.rect.h = round_dimension(old.h);
        self.snap_pos();
        SceneEvent::SpriteMove(self.id, old, self.rect)
    }

    pub fn enforce_min_size(&mut self) -> Option<SceneEvent> {
        if self.rect.w < Sprite::MIN_SIZE || self.rect.h < Sprite::MIN_SIZE {
            let from = self.rect;
            self.rect.w = self.rect.w.max(Sprite::MIN_SIZE);
            self.rect.h = self.rect.h.max(Sprite::MIN_SIZE);
            Some(SceneEvent::SpriteMove(self.id, from, self.rect))
        } else {
            None
        }
    }

    pub fn move_by(&mut self, delta: ScenePoint) -> SceneEvent {
        let from = self.rect;
        self.rect.translate(delta);
        SceneEvent::SpriteMove(self.id, from, self.rect)
    }

    pub fn pos(&self) -> ScenePoint {
        ScenePoint {
            x: self.rect.x,
            y: self.rect.y,
        }
    }

    pub fn anchor_point(&mut self, dx: i32, dy: i32) -> ScenePoint {
        let Rect { x, y, w, h } = self.rect;
        ScenePoint {
            x: x + (w / 2.0) * (dx + 1) as f32,
            y: y + (h / 2.0) * (dy + 1) as f32,
        }
    }

    pub fn set_colour(&mut self, new: Colour) -> Option<SceneEvent> {
        let old = self.visual.clone();
        match self.visual.clone() {
            Visual::Solid { shape, stroke, .. } => {
                self.visual = Visual::Solid {
                    colour: new,
                    shape,
                    stroke,
                };
                Some(SceneEvent::SpriteVisual(self.id, old, self.visual.clone()))
            }
            Visual::Drawing { points, stroke, .. } => {
                self.visual = Visual::Drawing {
                    colour: new,
                    points,
                    stroke,
                };
                Some(SceneEvent::SpriteVisual(self.id, old, self.visual.clone()))
            }
            _ => None,
        }
    }

    pub fn set_shape(&mut self, new: Shape) -> Option<SceneEvent> {
        let old = self.visual.clone();
        match self.visual.clone() {
            Visual::Solid { colour, stroke, .. } => {
                self.visual = Visual::Solid {
                    colour,
                    shape: new,
                    stroke,
                };
                Some(SceneEvent::SpriteVisual(self.id, old, self.visual.clone()))
            }
            Visual::Texture { id, shape: _ } => {
                self.visual = Visual::Texture { id, shape: new };
                Some(SceneEvent::SpriteVisual(self.id, old, self.visual.clone()))
            }
            _ => None,
        }
    }

    pub fn set_stroke(&mut self, new: f32) -> Option<SceneEvent> {
        let old = self.visual.clone();
        match self.visual.clone() {
            Visual::Solid { shape, colour, .. } => {
                self.visual = Visual::Solid {
                    shape,
                    stroke: new,
                    colour,
                };
                Some(SceneEvent::SpriteVisual(self.id, old, self.visual.clone()))
            }
            Visual::Drawing { colour, points, .. } => {
                self.visual = Visual::Drawing {
                    stroke: new,
                    colour,
                    points,
                };
                Some(SceneEvent::SpriteVisual(self.id, old, self.visual.clone()))
            }
            _ => None,
        }
    }

    pub fn set_texture(&mut self, new: Id) -> Option<SceneEvent> {
        if let Visual::Texture { id: _, shape } = self.visual {
            let old = self.visual.clone();
            self.visual = Visual::Texture { id: new, shape };
            Some(SceneEvent::SpriteVisual(self.id, old, self.visual.clone()))
        } else {
            None
        }
    }

    pub fn n_drawing_points(&self) -> usize {
        if let Visual::Drawing { points, .. } = &self.visual {
            points.len() / 2
        } else {
            0
        }
    }

    pub fn keep_drawing_points(&mut self, n: usize) {
        if let Visual::Drawing { points, .. } = &mut self.visual {
            points.truncate(n * 2);
        }
    }

    pub fn last_drawing_point(&self) -> Option<ScenePoint> {
        if let Visual::Drawing { points, .. } = &self.visual {
            if self.n_drawing_points() > 0 {
                let mut last: Vec<&f32> = points.iter().rev().take(2).collect();

                // These unwraps are safe because we checked that there is at
                // one point in the vec, implying at least two entries.
                let y = *last.pop().unwrap();
                let x = *last.pop().unwrap();
                Some(ScenePoint::new(x, y))
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn add_drawing_point(&mut self, at: ScenePoint) -> Option<SceneEvent> {
        let ScenePoint { x, y } = at - self.pos();
        if let Visual::Drawing { points, .. } = &mut self.visual {
            points.push(x);
            points.push(y);
            Some(SceneEvent::SpriteDrawingPoint(
                self.id,
                self.n_drawing_points(),
                at,
            ))
        } else {
            None
        }
    }

    pub fn calculate_drawing_rect(&mut self) -> Option<SceneEvent> {
        if let Visual::Drawing { points, .. } = &mut self.visual {
            let mut x_min = std::f32::MAX;
            let mut x_max = std::f32::MIN;
            let mut y_min = std::f32::MAX;
            let mut y_max = std::f32::MIN;

            for i in (0..points.len()).step_by(2) {
                let x = points[i];
                let y = points[i + 1];

                x_min = x_min.min(x);
                x_max = x_max.max(x);
                y_min = y_min.min(y);
                y_max = y_max.max(y);
            }

            self.rect = Rect::new(
                x_min + self.rect.x - Self::DEFAULT_STROKE,
                y_min + self.rect.y - Self::DEFAULT_STROKE,
                x_max - x_min + 2.0 * Self::DEFAULT_STROKE,
                y_max - y_min + 2.0 * Self::DEFAULT_STROKE,
            );

            for i in (0..points.len()).step_by(2) {
                points[i] -= x_min - Self::DEFAULT_STROKE;
                points[i + 1] -= y_min - Self::DEFAULT_STROKE;
            }

            Some(SceneEvent::SpriteDrawingFinish(self.id))
        } else {
            None
        }
    }
}

fn round_dimension(d: f32) -> f32 {
    let sign = d.signum();

    if d.abs() < 0.375 {
        sign * 0.25
    } else if d.abs() < 0.75 {
        sign * 0.5
    } else {
        d.round()
    }
}

fn determine_unit_size(d: f32) -> f32 {
    if d.abs() < 0.5 {
        0.25
    } else if d.abs() < 1.0 {
        0.5
    } else {
        1.0
    }
}

fn round_to_nearest(d: f32, n: f32) -> f32 {
    let sign = d.signum();
    let d = d.abs();
    sign * (d / n).round() * n
}
