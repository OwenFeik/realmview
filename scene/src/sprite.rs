use serde_derive::{Deserialize, Serialize};

use super::{comms::SceneEvent, Dimension, Id, Point, Rect};
use crate::rect::{determine_unit_size, float_eq};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Colour(pub [f32; 4]);

impl Colour {
    pub const DEFAULT: Colour = Colour([1.0, 0.0, 1.0, 1.0]);
    pub const RED: Colour = Colour([1.0, 0.0, 0.0, 1.0]);
    pub const GREEN: Colour = Colour([0.0, 1.0, 0.0, 1.0]);
    pub const BLUE: Colour = Colour([0.0, 0.0, 1.0, 1.0]);

    pub fn r(&self) -> f32 {
        self.0[0]
    }

    pub fn g(&self) -> f32 {
        self.0[1]
    }

    pub fn b(&self) -> f32 {
        self.0[2]
    }

    pub fn a(&self) -> f32 {
        self.0[3]
    }

    pub fn raw(self) -> [f32; 4] {
        self.0
    }

    pub fn arr(&self) -> &[f32; 4] {
        &self.0
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.0[3] = opacity;
        self
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum Shape {
    Ellipse,
    Hexagon,
    Rectangle,
    Triangle,
}

impl Shape {
    pub fn from(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "ellipse" => Self::Ellipse,
            "hexagon" => Self::Hexagon,
            "rectangle" => Self::Rectangle,
            "triangle" => Self::Triangle,
            _ => Self::Rectangle,
        }
    }

    pub fn to_str(&self) -> &'static str {
        match &self {
            Self::Ellipse => "ellipse",
            Self::Hexagon => "hexagon",
            Self::Rectangle => "rectangle",
            Self::Triangle => "triangle",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum DrawingMode {
    Freehand,
    Line,
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

    pub fn from(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "arrow" => Self::Arrow,
            "round" => Self::Round,
            "none" => Self::None,
            _ => Self::None,
        }
    }

    pub fn to_str(&self) -> &'static str {
        match &self {
            Self::Arrow => "arrow",
            Self::None => "none",
            Self::Round => "round",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum Visual {
    Texture {
        shape: Shape,
        id: Id,
    },
    Shape {
        shape: Shape,
        stroke: f32,
        solid: bool,
        colour: Colour,
    },
    Drawing {
        drawing: Id,
        mode: DrawingMode,
        colour: Colour,
        stroke: f32,
        cap_start: Cap,
        cap_end: Cap,
    },
}

impl Visual {
    pub fn new_shape(colour: Colour, shape: Shape, stroke: f32, solid: bool) -> Self {
        Visual::Shape {
            shape,
            stroke,
            solid: (solid || float_eq(stroke, Sprite::SOLID_STROKE)),
            colour,
        }
    }

    pub fn is_solid(&self) -> bool {
        self.solid().unwrap_or(false)
            || self
                .stroke()
                .map(|stroke| float_eq(stroke, Sprite::SOLID_STROKE))
                .unwrap_or(false)
    }

    pub fn colour(&self) -> Option<Colour> {
        match self {
            Self::Shape { colour, .. } | Self::Drawing { colour, .. } => Some(*colour),
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
            Self::Shape { shape, .. } | Self::Texture { id: _, shape } => Some(*shape),
            _ => None,
        }
    }

    pub fn drawing(&self) -> Option<Id> {
        if let Self::Drawing { drawing, .. } = self {
            Some(*drawing)
        } else {
            None
        }
    }

    pub fn stroke(&self) -> Option<f32> {
        match self {
            Self::Drawing { stroke, .. } | Self::Shape { stroke, .. } => Some(*stroke),
            _ => None,
        }
    }

    pub fn solid(&self) -> Option<bool> {
        if let Self::Shape { solid, .. } = self {
            Some(*solid)
        } else {
            None
        }
    }

    pub fn cap_start(&self) -> Option<Cap> {
        if let Self::Drawing { cap_start, .. } = self {
            Some(*cap_start)
        } else {
            None
        }
    }

    pub fn cap_end(&self) -> Option<Cap> {
        if let Self::Drawing { cap_end, .. } = self {
            Some(*cap_end)
        } else {
            None
        }
    }

    pub fn drawing_mode(&self) -> Option<DrawingMode> {
        if let Self::Drawing { mode, .. } = self {
            Some(*mode)
        } else {
            None
        }
    }
}

pub struct Outline {
    pub rect: Rect,
    pub shape: Shape,
}

impl Outline {
    pub fn rect(rect: Rect) -> Self {
        Self {
            rect,
            shape: Shape::Rectangle,
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
    pub const DEFAULT_WIDTH: f32 = 1.0;
    pub const DEFAULT_HEIGHT: f32 = 1.0;
    pub const DEFAULT_MODE: DrawingMode = DrawingMode::Freehand;

    // Minimum size of a sprite dimension; too small and sprites can be lost.
    const MIN_SIZE: f32 = 0.25;
    const DEFAULT_VISUAL: Visual = Visual::Shape {
        colour: Colour::DEFAULT,
        shape: Shape::Rectangle,
        stroke: Self::SOLID_STROKE,
        solid: false,
    };

    pub fn new(id: Id, visual: Option<Visual>) -> Self {
        Self {
            rect: Rect::new(0.0, 0.0, Self::DEFAULT_WIDTH, Self::DEFAULT_HEIGHT),
            z: 1,
            visual: visual.unwrap_or(Self::DEFAULT_VISUAL),
            id,
        }
    }

    pub fn set_rect(&mut self, rect: Rect) -> SceneEvent {
        let from = self.rect;
        self.rect = rect;

        SceneEvent::SpriteMove(self.id, from, self.rect)
    }

    pub fn set_pos(&mut self, pos: Point) -> SceneEvent {
        self.set_rect(self.rect.moved_to(pos))
    }

    pub fn set_dimension(&mut self, dimension: Dimension, value: f32) -> SceneEvent {
        self.set_rect(self.rect.dimension(dimension, value))
    }

    pub fn set_visual(&mut self, mut new: Visual) -> SceneEvent {
        std::mem::swap(&mut new, &mut self.visual);
        SceneEvent::SpriteVisual(self.id, new, self.visual.clone())
    }

    pub fn snap_pos(&mut self) -> SceneEvent {
        self.set_rect(self.rect.moved_to(Point::new(
            round_to_nearest(self.rect.x, determine_unit_size(self.rect.w)),
            round_to_nearest(self.rect.y, determine_unit_size(self.rect.h)),
        )))
    }

    pub fn snap_size(&mut self) -> SceneEvent {
        let old = self.rect;
        self.set_rect(
            self.rect
                .sized_as(round_dimension(self.rect.w), round_dimension(self.rect.h)),
        );
        self.snap_pos();
        SceneEvent::SpriteMove(self.id, old, self.rect)
    }

    pub fn enforce_min_size(&mut self) -> Option<SceneEvent> {
        let new_w = if self.rect.w.abs() < Sprite::MIN_SIZE {
            Some(self.rect.w.signum() * Sprite::MIN_SIZE)
        } else {
            None
        };

        let new_h = if self.rect.h.abs() < Sprite::MIN_SIZE {
            Some(self.rect.h.signum() * Sprite::MIN_SIZE)
        } else {
            None
        };

        if new_w.is_some() || new_h.is_some() {
            Some(
                self.set_rect(
                    self.rect
                        .sized_as(new_w.unwrap_or(self.rect.w), new_h.unwrap_or(self.rect.h)),
                ),
            )
        } else {
            None
        }
    }

    pub fn move_by(&mut self, delta: Point) -> SceneEvent {
        self.set_rect(self.rect.translate(delta))
    }

    pub fn pos(&self) -> Point {
        Point {
            x: self.rect.x,
            y: self.rect.y,
        }
    }

    pub fn anchor_point(&mut self, dx: i32, dy: i32) -> Point {
        let Rect { x, y, w, h } = self.rect;
        Point {
            x: x + (w / 2.0) * (dx + 1) as f32,
            y: y + (h / 2.0) * (dy + 1) as f32,
        }
    }

    pub fn set_colour(&mut self, new: Colour) -> Option<SceneEvent> {
        let old = self.visual.clone();
        match &mut self.visual {
            Visual::Shape { colour, .. } | Visual::Drawing { colour, .. } => {
                *colour = new;
                Some(SceneEvent::SpriteVisual(self.id, old, self.visual.clone()))
            }
            _ => None,
        }
    }

    pub fn set_shape(&mut self, new: Shape) -> Option<SceneEvent> {
        let old = self.visual.clone();
        match self.visual.clone() {
            Visual::Shape { colour, stroke, .. } => {
                self.visual = Visual::Shape {
                    colour,
                    shape: new,
                    stroke,
                    solid: old.is_solid(),
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
        match &mut self.visual {
            Visual::Shape { stroke, .. } | Visual::Drawing { stroke, .. } => {
                *stroke = new;
                Some(SceneEvent::SpriteVisual(self.id, old, self.visual.clone()))
            }
            _ => None,
        }
    }

    pub fn set_solid(&mut self, new: bool) -> Option<SceneEvent> {
        let old = self.visual.clone();
        match &mut self.visual {
            Visual::Shape { solid, .. } => {
                *solid = new;
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

    pub fn set_drawing_type(&mut self, new: DrawingMode) -> Option<SceneEvent> {
        let old = self.visual.clone();
        if let Visual::Drawing { mode, .. } = &mut self.visual {
            *mode = new;
            Some(SceneEvent::SpriteVisual(self.id, old, self.visual.clone()))
        } else {
            None
        }
    }

    pub fn set_caps(&mut self, start: Option<Cap>, end: Option<Cap>) -> Option<SceneEvent> {
        let before = self.visual.clone();
        if let Visual::Drawing {
            cap_start, cap_end, ..
        } = &mut self.visual
        {
            if let Some(cap) = start {
                *cap_start = cap;
            }
            if let Some(cap) = end {
                *cap_end = cap;
            }

            Some(SceneEvent::SpriteVisual(
                self.id,
                before,
                self.visual.clone(),
            ))
        } else {
            None
        }
    }

    pub fn drawing_finished(&mut self, rect: Rect) {
        if let Visual::Drawing { .. } = &mut self.visual {
            self.rect = rect;
        }
    }

    pub fn outline(&self) -> Outline {
        let rect = self.rect;
        match self.visual {
            Visual::Drawing { stroke, .. } => Outline {
                rect: Rect {
                    x: self.rect.x - stroke,
                    y: self.rect.y - stroke,
                    w: self.rect.w + stroke * 2.0,
                    h: self.rect.h + stroke * 2.0,
                },
                shape: Shape::Rectangle,
            },
            _ => Outline {
                rect,
                shape: Shape::Rectangle,
            },
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

fn round_to_nearest(d: f32, n: f32) -> f32 {
    let sign = d.signum();
    let d = d.abs();
    sign * (d / n).round() * n
}

#[cfg(test)]
mod test {
    use super::round_dimension;
    use crate::{rect::float_eq, sprite::round_to_nearest};

    #[test]
    fn test_round_dimension() {
        assert!(float_eq(round_dimension(-123.456), -123.0));
        assert!(float_eq(round_dimension(0.1), 0.25));
        assert!(float_eq(round_dimension(-0.6), -0.5));
        assert!(float_eq(round_dimension(0.76), 1.0));
    }

    #[test]
    fn test_round_to_nearest() {
        assert!(float_eq(round_to_nearest(0.66, 0.25), 0.75));
        assert!(float_eq(round_to_nearest(0.74, 0.5), 0.5));
        assert!(float_eq(round_to_nearest(5.2, 3.0), 6.0));
    }
}
