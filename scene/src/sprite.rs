use std::sync::atomic::{AtomicI64, Ordering};

use serde_derive::{Deserialize, Serialize};

use super::{comms::SceneEvent, HeldObject, Id, Rect, ScenePoint};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Sprite {
    pub rect: Rect,

    pub z: i32,

    // id pointing to the texture associated with this Sprite
    pub texture: Id,

    // Unique numeric ID, numbered from 1
    pub local_id: Id,

    // ID of the Sprite on the server side
    pub canonical_id: Option<Id>,
}

impl Sprite {
    // Distance in scene units from which anchor points (corners, edges) of the
    // sprite can be dragged.
    const ANCHOR_RADIUS: f32 = 0.2;

    // Minimum size of a sprite dimension; too small and sprites can be lost.
    const MIN_SIZE: f32 = 0.25;

    pub fn new(texture: Id) -> Sprite {
        static SPRITE_ID: AtomicI64 = AtomicI64::new(1);

        Sprite {
            rect: Rect::new(0.0, 0.0, 1.0, 1.0),
            z: 1,
            texture,
            local_id: SPRITE_ID.fetch_add(1, Ordering::Relaxed),
            canonical_id: None,
        }
    }

    pub fn from_remote(sprite: &Sprite) -> Sprite {
        let mut new = Sprite::new(sprite.texture);
        new.set_rect(sprite.rect);
        new.z = sprite.z;
        new.canonical_id = sprite.canonical_id;
        new
    }

    fn set_pos(&mut self, ScenePoint { x, y }: ScenePoint) -> Option<SceneEvent> {
        let from = self.rect;
        self.rect.x = x;
        self.rect.y = y;

        self.canonical_id
            .map(|id| SceneEvent::SpriteMove(id, from, self.rect))
    }

    pub fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn set_size(&mut self, w: f32, h: f32) {
        self.rect.w = w;
        self.rect.h = h;
    }

    pub fn set_texture(&mut self, new: Id) {
        self.texture = new;
    }

    pub fn snap_to_grid(&mut self) -> Option<SceneEvent> {
        let from = self.rect;
        self.rect.round();
        self.canonical_id
            .map(|id| SceneEvent::SpriteMove(id, from, self.rect))
    }

    pub fn enforce_min_size(&mut self) -> Option<SceneEvent> {
        if self.rect.w < Sprite::MIN_SIZE || self.rect.h < Sprite::MIN_SIZE {
            let from = self.rect;
            self.rect.w = self.rect.w.max(Sprite::MIN_SIZE);
            self.rect.h = self.rect.h.max(Sprite::MIN_SIZE);
            self.canonical_id
                .map(|id| SceneEvent::SpriteMove(id, from, self.rect))
        } else {
            None
        }
    }

    fn grab_anchor(&mut self, at: ScenePoint) -> Option<HeldObject> {
        let Rect { x, y, w, h } = self.rect;

        // Anchor size is 0.2 tiles or one fifth of the smallest dimension of
        // the sprite. This is to allow sprites that are ANCHOR_RADIUS or
        // smaller to nonetheless be grabbed.
        let mut closest_dist = Sprite::ANCHOR_RADIUS.min(w.abs().min(h.abs()) / 5.0);
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
            Some(HeldObject::Anchor(self.local_id, closest.0, closest.1))
        } else {
            None
        }
    }

    pub fn grab(&mut self, at: ScenePoint) -> HeldObject {
        self.grab_anchor(at).unwrap_or({
            HeldObject::Sprite(
                self.local_id,
                ScenePoint {
                    x: at.x - self.rect.x,
                    y: at.y - self.rect.y,
                },
            )
        })
    }

    pub fn pos(&self) -> ScenePoint {
        ScenePoint {
            x: self.rect.x,
            y: self.rect.y,
        }
    }

    fn anchor_point(&mut self, dx: i32, dy: i32) -> ScenePoint {
        let Rect { x, y, w, h } = self.rect;
        ScenePoint {
            x: x + (w / 2.0) * (dx + 1) as f32,
            y: y + (h / 2.0) * (dy + 1) as f32,
        }
    }

    pub fn update_held_pos(&mut self, holding: HeldObject, at: ScenePoint) -> Option<SceneEvent> {
        match holding {
            HeldObject::Sprite(_, offset) => self.set_pos(at - offset),
            HeldObject::Anchor(_, dx, dy) => {
                let old_rect = self.rect;

                let ScenePoint {
                    x: delta_x,
                    y: delta_y,
                } = at - self.anchor_point(dx, dy);
                let x = self.rect.x + (if dx == -1 { delta_x } else { 0.0 });
                let y = self.rect.y + (if dy == -1 { delta_y } else { 0.0 });
                let w = delta_x * (dx as f32) + self.rect.w;
                let h = delta_y * (dy as f32) + self.rect.h;

                self.rect = Rect { x, y, w, h };
                self.canonical_id
                    .map(|id| SceneEvent::SpriteMove(id, old_rect, self.rect))
            }
            HeldObject::None => None, // Other types aren't sprite-related
        }
    }
}
