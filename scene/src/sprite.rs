use serde_derive::{Deserialize, Serialize};

use super::{comms::SceneEvent, Dimension, Id, Point, Rect};
use crate::rect::determine_unit_size;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct Colour(pub [f32; 4]);

impl Colour {
    pub const DEFAULT: Colour = Colour([1.0, 0.0, 1.0, 1.0]);

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
        drawing: Id,
        mode: DrawingMode,
        colour: Colour,
        stroke: f32,
        cap_start: Cap,
        cap_end: Cap,
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

    pub fn drawing(&self) -> Option<Id> {
        if let Self::Drawing { drawing, .. } = self {
            Some(*drawing)
        } else {
            None
        }
    }

    pub fn stroke(&self) -> Option<f32> {
        match self {
            Self::Drawing { stroke, .. } | Self::Solid { stroke, .. } => Some(*stroke),
            _ => None,
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
    const DEFAULT_VISUAL: Visual = Visual::Solid {
        colour: Colour::DEFAULT,
        shape: Shape::Rectangle,
        stroke: Self::SOLID_STROKE,
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
        let mut new_w = None;
        let mut new_h = None;
        if self.rect.w.abs() < Sprite::MIN_SIZE {
            new_w = Some(self.rect.w.signum() * Sprite::MIN_SIZE);
        }

        if self.rect.h.abs() < Sprite::MIN_SIZE {
            new_h = Some(self.rect.w.signum() * Sprite::MIN_SIZE);
        }

        if new_w.is_some() || new_h.is_some() {
            Some(
                self.set_rect(
                    self.rect
                        .sized_as(new_w.unwrap_or(self.rect.w), new_w.unwrap_or(self.rect.h)),
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
            Visual::Solid { colour, .. } | Visual::Drawing { colour, .. } => {
                *colour = new;
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
        match &mut self.visual {
            Visual::Solid { stroke, .. } | Visual::Drawing { stroke, .. } => {
                *stroke = new;
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
        if let Visual::Drawing { stroke, .. } = &mut self.visual {
            let d = *stroke;

            // Move the sprite to allow for the new position and border
            self.rect.x += rect.x - d;
            self.rect.y += rect.y - d;
            self.rect.w = rect.w + 2.0 * d;
            self.rect.h = rect.h + 2.0 * d;
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

/*
#[cfg(test)]
mod test {
    use super::{round_dimension, Cap, Colour, Drawing, DrawingMode, Sprite, Visual};
    use crate::{rect::float_eq, Point, PointVector, Rect};

    #[test]
    fn test_simplify() {
        let mut drawing = Drawing::new();
        drawing.add_point(Point::same(-1.0));
        drawing.add_point(Point::same(0.0));
        drawing.add_point(Point::same(2.0));
        drawing.simplify();
        drawing.translate(0.5);
        assert_eq!(
            drawing.points,
            PointVector::from(vec![1.5, 1.5, 0.5, 0.5, 1.5, 1.5, 3.5, 3.5])
        );
    }

    // Debugging issue where sprites with square caps jump a little when
    // finishing some of the time. Turns out this is a visual bug, the logic
    // for finishing drawings is unaffected by cap style.
    #[test]
    fn test_finish_drawing() {
        let mut a = Sprite {
            id: 1,
            rect: Rect {
                x: 12.93541,
                y: 31.636553,
                w: 1.0,
                h: 1.0,
            },
            z: 1,
            visual: Visual::Drawing(Drawing {
                points: PointVector {
                    data: [
                        0.0,
                        0.0,
                        -0.34547806,
                        -0.12562943,
                        -0.5967331,
                        -0.21985054,
                        -0.7851753,
                        -0.28266335,
                        -1.0050259,
                        -0.37688446,
                        -1.2248745,
                        -0.502512,
                        -1.350502,
                        -0.5339203,
                        -1.601759,
                        -0.6909542,
                        -1.7587929,
                        -0.78517723,
                        -1.9786434,
                        -0.87939644,
                        -2.104271,
                        -0.94221115,
                        -2.2298985,
                        -1.0364323,
                        -2.3555279,
                        -1.0678387,
                        -2.4811554,
                        -1.099247,
                    ]
                    .to_vec(),
                },
                drawing_type: DrawingMode::Freehand,
                colour: Colour([0.7002602, 0.9896201, 0.7272567, 1.0]),
                stroke: 0.2,
                cap_start: Cap::Round,
                cap_end: Cap::None,
                finished: false,
            }),
        };

        let mut b = a.clone();
        b.set_caps(None, Some(Cap::Round));

        a.finish_drawing();
        b.finish_drawing();

        if let (Visual::Drawing(a), Visual::Drawing(b)) = (a.visual, b.visual) {
            assert_eq!(a.points.data, b.points.data);
        }
    }

    #[test]
    fn test_round_dimension() {
        assert!(float_eq(round_dimension(-123.456), -123.0));
        assert!(float_eq(round_dimension(0.1), 0.25));
        assert!(float_eq(round_dimension(-0.6), -0.5));
        assert!(float_eq(round_dimension(0.76), 1.0));
    }
}
*/
