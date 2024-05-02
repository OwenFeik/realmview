use scene::{Id, Point, Rect, Sprite};

use crate::bridge::Cursor;

#[derive(Clone, Debug)]
pub enum HeldObject {
    /// (sprite, dx, dy, starting_rect, ephemeral)
    Anchor(Id, i32, i32, Rect, bool),
    /// (sprite, centre, ephemeral)
    Circle(Id, Point, bool),
    /// (drawing, sprite, ephemeral, measurement)
    Drawing(Id, Id, bool, bool),
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
            Self::Anchor(id, ..)
            | Self::Circle(id, _, _)
            | Self::Drawing(_, id, ..)
            | Self::Sprite(id, ..) => Some(*id),
            _ => None,
        }
    }

    fn is_none(&self) -> bool {
        matches!(self, HeldObject::None)
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

        closest.map(|(dx, dy)| Self::Anchor(sprite.id, dx, dy, sprite.rect, false))
    }

    pub fn sprite(sprite: &Sprite, at: Point) -> Self {
        Self::Sprite(sprite.id, at - sprite.rect.top_left(), sprite.rect)
    }

    pub fn grab_sprite(sprite: &Sprite, at: Point) -> Self {
        Self::grab_sprite_anchor(sprite, at).unwrap_or_else(|| Self::sprite(sprite, at))
    }

    pub fn cursor(&self, at: Point) -> Cursor {
        match self {
            Self::Anchor(_, dx, dy, Rect { w, h, .. }, _) => match (dx, dy) {
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
            Self::Circle(_, centre, _) => {
                let theta = centre.angle(at);
                Cursor::for_angle(theta)
            }
            Self::Drawing(..) => Cursor::Crosshair,
            Self::Marquee(..) | Self::None => Cursor::Default,
            Self::Selection(..) | Self::Sprite(..) => Cursor::Move,
        }
    }
}

#[cfg(test)]
mod test {
    use scene::Point;

    use super::HeldObject;
    use crate::bridge::Cursor;

    #[test]
    fn test_holding_circle_directions() {
        let h = HeldObject::Circle(1, Point::ORIGIN, true);
        assert_eq!(h.cursor(Point::new(10., 0.1)), Cursor::EwResize);
        assert_eq!(h.cursor(Point::new(3., 3.)), Cursor::NwseResize);
        assert_eq!(h.cursor(Point::new(0.1, 10.)), Cursor::NsResize);
        assert_eq!(h.cursor(Point::new(-0.1, 10.)), Cursor::NsResize);
        assert_eq!(h.cursor(Point::new(-3., 3.)), Cursor::NeswResize);
        assert_eq!(h.cursor(Point::new(-10., 0.1)), Cursor::EwResize);
        assert_eq!(h.cursor(Point::new(-10., -0.1)), Cursor::EwResize);
        assert_eq!(h.cursor(Point::new(-3., -3.)), Cursor::NwseResize);
        assert_eq!(h.cursor(Point::new(-0.1, -10.)), Cursor::NsResize);
        assert_eq!(h.cursor(Point::new(0.1, -10.)), Cursor::NsResize);
        assert_eq!(h.cursor(Point::new(3., -3.)), Cursor::NeswResize);
        assert_eq!(h.cursor(Point::new(10., -0.1)), Cursor::EwResize);
    }
}
