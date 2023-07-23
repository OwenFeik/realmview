use scene::{Id, Point, Rect, Sprite};

use crate::bridge::Cursor;

#[derive(Clone, Debug)]
pub enum HeldObject {
    /// (sprite, dx, dy, starting_rect)
    Anchor(Id, i32, i32, Rect),

    /// (drawing, sprite)
    Drawing(Id, Id),
    Ephemeral(Box<HeldObject>),
    Marquee(Point),
    None,
    Selection(Point),

    /// (sprite, delta, starting_rect)
    Sprite(Id, Point, Rect),
}

impl HeldObject {
    // Distance in scene units from which anchor points (corners, edges) of the
    // sprite can be dragged.
    pub const ANCHOR_RADIUS: f32 = 0.2;

    pub fn held_id(&self) -> Option<Id> {
        match self {
            Self::Anchor(id, ..) | Self::Drawing(id, ..) | Self::Sprite(id, ..) => Some(*id),
            Self::Ephemeral(held) => held.held_id(),
            _ => None,
        }
    }

    fn is_none(&self) -> bool {
        matches!(self, HeldObject::None)
    }

    /// If this isn't a HeldObject::Ephemeral, wrap it in one.
    pub fn ephemeral(&mut self) {
        if !matches!(self, Self::Ephemeral(..)) {
            *self = HeldObject::Ephemeral(Box::new(self.clone()))
        }
    }

    /// Update this HeldObject so that it is wrapped in HeldObject::Ephemeral
    /// if ephemeral is true, otherwise not wrapped.
    pub fn set_ephemeral(&mut self, ephemeral: bool) {
        if let Self::Ephemeral(held) = self {
            if !ephemeral {
                *self = *held.clone();
            }
        } else if ephemeral {
            self.ephemeral();
        }
    }

    pub fn is_sprite(&self) -> bool {
        matches!(
            self,
            HeldObject::Anchor(..) | HeldObject::Selection(..) | HeldObject::Sprite(..)
        )
    }

    pub fn anchors(sprite: &Sprite) -> impl Iterator<Item = Point> {
        const ANCHORS: &[Point] = &[
            Point::new(0.0, 0.0),
            Point::new(0.0, 1.0),
            Point::new(0.0, 2.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 2.0),
            Point::new(2.0, 0.0),
            Point::new(2.0, 1.0),
            Point::new(2.0, 2.0),
        ];

        let translation = sprite.rect.top_left();
        let scaling = sprite.rect.dimensions() / 2.0;
        ANCHORS
            .iter()
            .map(move |&delta| translation + scaling * delta)
    }

    fn grab_sprite_anchor(sprite: &Sprite, at: Point) -> Option<Self> {
        let Rect { x, y, w, h } = sprite.rect;

        let mut closest_dist = Self::ANCHOR_RADIUS;
        let mut closest = None;
        for dx in -1..2 {
            for dy in -1..2 {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let anchor_x = x + (w / 2.0) * (dx + 1) as f32;
                let anchor_y = y + (h / 2.0) * (dy + 1) as f32;

                let anchor = Point::new(anchor_x, anchor_y);
                let dist = at.dist(anchor);
                if dist <= closest_dist {
                    closest = Some((dx, dy));
                    closest_dist = dist;
                }
            }
        }

        closest.map(|(dx, dy)| Self::Anchor(sprite.id, dx, dy, sprite.rect))
    }

    pub fn sprite(sprite: &Sprite, at: Point) -> Self {
        Self::Sprite(sprite.id, at - sprite.rect.top_left(), sprite.rect)
    }

    pub fn grab_sprite(sprite: &Sprite, at: Point) -> Self {
        Self::grab_sprite_anchor(sprite, at).unwrap_or_else(|| Self::sprite(sprite, at))
    }

    pub fn cursor(&self) -> Cursor {
        match self {
            Self::Anchor(_, dx, dy, Rect { w, h, .. }) => match (dx, dy) {
                (-1, -1) | (1, 1) => {
                    if w.signum() == h.signum() {
                        Cursor::NwseResize
                    } else {
                        Cursor::NeswResize
                    }
                }
                (-1, 1) | (1, -1) => {
                    if w.signum() == h.signum() {
                        Cursor::NeswResize
                    } else {
                        Cursor::NwseResize
                    }
                }
                (0, -1) | (0, 1) => Cursor::NsResize,
                (-1, 0) | (1, 0) => Cursor::EwResize,
                _ => Cursor::Move,
            },
            Self::Drawing(..) => Cursor::Crosshair,
            Self::Ephemeral(held) => held.cursor(),
            Self::Marquee(..) | Self::None => Cursor::Default,
            Self::Selection(..) | Self::Sprite(..) => Cursor::Move,
        }
    }

    /// Returns a clone of this HeldObject if it isn't a wrapper, otherwise
    /// a clone of the wrapped HeldObject.
    pub fn value(&self) -> HeldObject {
        match self {
            HeldObject::Ephemeral(held) => *held.clone(),
            _ => self.clone(),
        }
    }
}
