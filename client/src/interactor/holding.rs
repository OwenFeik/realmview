use scene::{Id, Point, Rect, Sprite};

use crate::bridge::Cursor;

#[derive(Clone, Debug)]
pub enum HeldObject {
    Anchor(Id, i32, i32, Rect), // (sprite, dx, dy, starting_rect)
    Drawing(Id),                // (sprite, ephemeral)
    Ephemeral(Box<HeldObject>),
    Marquee(Point),
    None,
    Selection(Point),
    Sprite(Id, Point, Rect), // (sprite, delta, starting_rect)
}

impl HeldObject {
    // Distance in scene units from which anchor points (corners, edges) of the
    // sprite can be dragged.
    const ANCHOR_RADIUS: f32 = 0.2;

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

    fn grab_sprite_anchor(sprite: &Sprite, at: Point) -> Option<Self> {
        let Rect { x, y, w, h } = sprite.rect;

        // Anchor size is 0.2 tiles or one fifth of the smallest dimension of
        // the sprite. This is to allow sprites that are ANCHOR_RADIUS or
        // smaller to nonetheless be grabbed.
        let mut closest_dist = Self::ANCHOR_RADIUS.min(w.abs().min(h.abs()) / 5.0);
        let mut closest: (i32, i32) = (2, 2);
        for dx in -1..2 {
            for dy in -1..2 {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let anchor_x = x + (w / 2.0) * (dx + 1) as f32;
                let anchor_y = y + (h / 2.0) * (dy + 1) as f32;

                let delta_x = anchor_x - at.x;
                let delta_y = anchor_y - at.y;

                let dist = (delta_x.powi(2) + delta_y.powi(2)).sqrt();
                if dist <= closest_dist {
                    closest = (dx, dy);
                    closest_dist = dist;
                }
            }
        }

        if closest != (2, 2) {
            Some(Self::Anchor(sprite.id, closest.0, closest.1, sprite.rect))
        } else {
            None
        }
    }

    pub fn grab_sprite(sprite: &Sprite, at: Point) -> Self {
        Self::grab_sprite_anchor(sprite, at)
            .unwrap_or_else(|| Self::Sprite(sprite.id, at - sprite.rect.top_left(), sprite.rect))
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
