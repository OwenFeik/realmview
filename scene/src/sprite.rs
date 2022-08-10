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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum Cap {
    Arrow,
    None,
    Round,
}

impl Cap {
    pub const DEFAULT_START: Cap = Cap::Round;
    pub const DEFAULT_END: Cap = Cap::Arrow;
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Drawing {
    pub points: Vec<f32>,
    pub cap_start: Cap,
    pub cap_end: Cap,
    prev_point: Option<ScenePoint>,
}

impl Drawing {
    pub fn new(points: Vec<f32>, cap_start: Cap, cap_end: Cap) -> Self {
        Self {
            points,
            cap_start,
            cap_end,
            ..Default::default()
        }
    }

    pub fn n_points(&self) -> usize {
        self.points.len() / 2
    }

    fn keep_n_points(&mut self, n: usize) {
        self.points.truncate(n * 2);
    }

    fn last_point(&self) -> Option<ScenePoint> {
        if self.n_points() > 0 {
            let mut last: Vec<&f32> = self.points.iter().rev().take(2).collect();

            // These unwraps are safe because we checked that there is at
            // one point in the vec, implying at least two entries.
            let y = *last.pop().unwrap();
            let x = *last.pop().unwrap();
            Some(ScenePoint::new(x, y))
        } else {
            None
        }
    }

    // Adds a new point to the drawing, if it isn't too close to the previous
    // point.
    fn add_point(&mut self, point: ScenePoint) {
        const MINIMUM_DISTANCE: f32 = 0.1;

        if let Some(prev) = self.prev_point {
            if prev.dist(point) < MINIMUM_DISTANCE {
                return;
            }
        }

        self.points.push(point.x);
        self.points.push(point.y);
    }

    fn rect(&mut self) -> Rect {
        let mut x_min = std::f32::MAX;
        let mut x_max = std::f32::MIN;
        let mut y_min = std::f32::MAX;
        let mut y_max = std::f32::MIN;

        let points = &self.points;
        for i in (0..points.len()).step_by(2) {
            let x = points[i];
            let y = points[i + 1];

            x_min = x.min(x_min);
            x_max = x.max(x_max);
            y_min = y.min(y_min);
            y_max = y.max(y_max);
        }

        Rect::new(x_min, y_min, x_max - x_min, y_max - y_min)
    }

    // Simplifies the drawing's points so that the top-left-most point is the
    // origin, returning a ScenePoint indicating the change in position of the
    // top-left-most point.
    fn simplify(&mut self) -> ScenePoint {
        let rect = self.rect();

        for i in (0..self.points.len()).step_by(2) {
            self.points[i] -= rect.x;
            self.points[i + 1] -= rect.y;
        }

        rect.top_left()
    }
}

impl Default for Drawing {
    fn default() -> Self {
        Self {
            points: vec![0.0, 0.0],
            cap_start: Cap::Round,
            cap_end: Cap::Round,
            prev_point: None,
        }
    }
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
        drawing: Drawing,
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

    pub fn drawing(&self) -> Option<&Drawing> {
        if let Self::Drawing { drawing, .. } = self {
            Some(drawing)
        } else {
            None
        }
    }

    pub fn stroke(&self) -> Option<f32> {
        match self {
            Self::Solid { stroke, .. } | Self::Drawing { stroke, .. } => Some(*stroke),
            _ => None,
        }
    }

    pub fn cap_start(&self) -> Option<Cap> {
        if let Self::Drawing { drawing, .. } = self {
            Some(drawing.cap_start)
        } else {
            None
        }
    }

    pub fn cap_end(&self) -> Option<Cap> {
        if let Self::Drawing { drawing, .. } = self {
            Some(drawing.cap_end)
        } else {
            None
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
    pub const DEFAULT_COLOUR: Colour = [1.0, 0.0, 1.0, 1.0];

    // Minimum size of a sprite dimension; too small and sprites can be lost.
    const MIN_SIZE: f32 = 0.25;
    const DEFAULT_VISUAL: Visual = Visual::Solid {
        colour: Self::DEFAULT_COLOUR,
        shape: Shape::Rectangle,
        stroke: Self::SOLID_STROKE,
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
            Visual::Drawing {
                stroke,
                colour: _,
                drawing,
            } => {
                self.visual = Visual::Drawing {
                    stroke,
                    colour: new,
                    drawing,
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
            Visual::Drawing {
                stroke: _,
                colour,
                drawing,
            } => {
                self.visual = Visual::Drawing {
                    stroke: new,
                    colour,
                    drawing,
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

    pub fn set_caps(&mut self, start: Option<Cap>, end: Option<Cap>) -> Option<SceneEvent> {
        let before = self.visual.clone();
        if let Visual::Drawing { drawing, .. } = &mut self.visual {
            drawing.cap_start = start.unwrap_or(drawing.cap_start);
            drawing.cap_end = end.unwrap_or(drawing.cap_end);
            Some(SceneEvent::SpriteVisual(
                self.id,
                before,
                self.visual.clone(),
            ))
        } else {
            None
        }
    }

    pub fn n_drawing_points(&self) -> usize {
        if let Visual::Drawing { drawing, .. } = &self.visual {
            drawing.n_points()
        } else {
            0
        }
    }

    pub fn keep_drawing_points(&mut self, n: usize) {
        if let Visual::Drawing { drawing, .. } = &mut self.visual {
            drawing.keep_n_points(n * 2);
        }
    }

    pub fn last_drawing_point(&self) -> Option<ScenePoint> {
        if let Visual::Drawing { drawing, .. } = &self.visual {
            drawing.last_point()
        } else {
            None
        }
    }

    pub fn add_drawing_point(&mut self, at: ScenePoint) -> Option<SceneEvent> {
        let point = at - self.pos();
        if let Visual::Drawing { drawing, .. } = &mut self.visual {
            drawing.add_point(point);
            Some(SceneEvent::SpriteDrawingPoint(
                self.id,
                drawing.n_points(),
                at,
            ))
        } else {
            None
        }
    }

    pub fn finish_drawing(&mut self) -> Option<SceneEvent> {
        if let Visual::Drawing {
            stroke, drawing, ..
        } = &mut self.visual
        {
            let stroke = *stroke;
            let dpos = drawing.simplify();
            let Rect { x: _, y: _, w, h } = drawing.rect();
            self.rect.translate(dpos);
            self.rect.w = w + 2.0 * stroke;
            self.rect.h = h + 2.0 * stroke;
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
