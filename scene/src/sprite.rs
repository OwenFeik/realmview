use std::sync::atomic::{AtomicI64, Ordering};

use serde_derive::{Deserialize, Serialize};

use super::{comms::SceneEvent, Id, Rect, ScenePoint};

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

    fn event<F: Fn(Id) -> SceneEvent>(&self, clos: F) -> Option<SceneEvent> {
        self.canonical_id.map(clos)
    }

    pub fn from_remote(sprite: &Sprite) -> Sprite {
        let mut new = Sprite::new(sprite.texture);
        new.set_rect(sprite.rect);
        new.z = sprite.z;
        new.canonical_id = sprite.canonical_id;
        new
    }

    pub fn set_pos(&mut self, ScenePoint { x, y }: ScenePoint) -> Option<SceneEvent> {
        let from = self.rect;
        self.rect.x = x;
        self.rect.y = y;

        self.event(|id| SceneEvent::SpriteMove(id, from, self.rect))
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
        self.event(|id| SceneEvent::SpriteMove(id, from, self.rect))
    }

    pub fn enforce_min_size(&mut self) -> Option<SceneEvent> {
        if self.rect.w < Sprite::MIN_SIZE || self.rect.h < Sprite::MIN_SIZE {
            let from = self.rect;
            self.rect.w = self.rect.w.max(Sprite::MIN_SIZE);
            self.rect.h = self.rect.h.max(Sprite::MIN_SIZE);
            self.event(|id| SceneEvent::SpriteMove(id, from, self.rect))
        } else {
            None
        }
    }

    pub fn move_by(&mut self, delta: ScenePoint) -> Option<SceneEvent> {
        let old = self.rect;
        self.rect.translate(delta);
        self.event(|id| SceneEvent::SpriteMove(id, old, self.rect))
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
}
