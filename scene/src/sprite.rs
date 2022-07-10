use serde_derive::{Deserialize, Serialize};

use crate::Dimension;

use super::{comms::SceneEvent, Id, Rect, ScenePoint};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Sprite {
    pub id: Id,
    pub rect: Rect,
    pub z: i32,

    // id pointing to the texture associated with this Sprite
    pub texture: Id,
}

impl Sprite {
    // Minimum size of a sprite dimension; too small and sprites can be lost.
    const MIN_SIZE: f32 = 0.25;

    pub fn new(id: Id, texture: Id) -> Sprite {
        Sprite {
            rect: Rect::new(0.0, 0.0, 1.0, 1.0),
            z: 1,
            texture,
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

    pub fn set_texture(&mut self, new: Id) -> SceneEvent {
        let old = self.texture;
        self.texture = new;
        SceneEvent::SpriteTexture(self.id, old, new)
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
