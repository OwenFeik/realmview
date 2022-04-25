#![allow(dead_code)]

use serde_derive::{Deserialize, Serialize};
use std::ops::{Add, Sub};
use std::sync::atomic::{AtomicI64, Ordering};

pub mod comms;

use comms::SceneEvent;

pub type Id = i64;

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    pub fn from_point(point: ScenePoint, w: f32, h: f32) -> Rect {
        Rect {
            x: point.x,
            y: point.y,
            w,
            h,
        }
    }

    pub fn scaled_from(from: Rect, factor: f32) -> Rect {
        let mut rect = from;
        rect.scale(factor);
        rect
    }

    pub fn as_floats(&self) -> (f32, f32, f32, f32) {
        (self.x as f32, self.y as f32, self.w as f32, self.h as f32)
    }

    fn scale(&mut self, factor: f32) {
        self.x *= factor;
        self.y *= factor;
        self.w *= factor;
        self.h *= factor;
    }

    fn round(&mut self) {
        self.x = self.x.round();
        self.y = self.y.round();
        self.w = self.w.round();
        self.h = self.h.round();

        if self.w > 0.0 && self.w < 1.0 {
            self.w = 1.0;
        } else if self.w < 0.0 && self.w > -1.0 {
            self.w = -1.0;
        }

        if self.h > 0.0 && self.h < 1.0 {
            self.h = 1.0;
        } else if self.h < 0.0 && self.h > -1.0 {
            self.h = -1.0;
        }
    }

    fn contains_point(&self, point: ScenePoint) -> bool {
        // A negative dimension causes a texture to be flipped. As this is a useful behaviour, negative dimensions on
        // Rects are supported. To that end a different treatment is required for checking if a point is contained.
        // Hence the special cases for negative width and height.

        let in_x = {
            if self.w < 0.0 {
                self.x + self.w <= point.x && point.x <= self.x
            } else {
                self.x <= point.x && point.x <= self.x + self.w
            }
        };

        let in_y = {
            if self.h < 0.0 {
                self.y + self.h <= point.y && point.y <= self.y
            } else {
                self.y <= point.y && point.y <= self.y + self.h
            }
        };

        in_x && in_y
    }

    pub fn top_left(&self) -> ScenePoint {
        ScenePoint {
            x: self.x,
            y: self.y,
        }
    }
}

impl Add for Rect {
    type Output = Rect;

    fn add(self, rhs: Rect) -> Rect {
        Rect {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            w: self.w + rhs.w,
            h: self.h + rhs.h,
        }
    }
}

impl Sub for Rect {
    type Output = Rect;

    fn sub(self, rhs: Rect) -> Rect {
        Rect {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            w: self.w - rhs.w,
            h: self.h - rhs.h,
        }
    }
}

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

    fn from_remote(sprite: &Sprite) -> Sprite {
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

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn set_size(&mut self, w: f32, h: f32) {
        self.rect.w = w;
        self.rect.h = h;
    }

    fn set_texture(&mut self, new: Id) {
        self.texture = new;
    }

    fn snap_to_grid(&mut self) -> Option<SceneEvent> {
        let from = self.rect;
        self.rect.round();
        self.canonical_id
            .map(|id| SceneEvent::SpriteMove(id, from, self.rect))
    }

    fn grab_anchor(&mut self, at: ScenePoint) -> Option<HeldObject> {
        let Rect { x, y, w, h } = self.rect;

        for dx in -1..2 {
            for dy in -1..2 {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let anchor_x = x + (w / 2.0) * (dx + 1) as f32;
                let anchor_y = y + (h / 2.0) * (dy + 1) as f32;

                let delta_x = anchor_x - at.x;
                let delta_y = anchor_y - at.y;

                if (delta_x.powi(2) + delta_y.powi(2)).sqrt() <= Sprite::ANCHOR_RADIUS {
                    return Some(HeldObject::Anchor(self.local_id, dx, dy));
                }
            }
        }

        None
    }

    fn grab(&mut self, at: ScenePoint) -> HeldObject {
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

    fn update_held_pos(&mut self, holding: HeldObject, at: ScenePoint) -> Option<SceneEvent> {
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

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq)]
pub struct ScenePoint {
    pub x: f32,
    pub y: f32,
}

impl ScenePoint {
    pub fn new(x: f32, y: f32) -> ScenePoint {
        ScenePoint { x, y }
    }
}

impl Add for ScenePoint {
    type Output = ScenePoint;

    fn add(self, rhs: ScenePoint) -> ScenePoint {
        ScenePoint {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for ScenePoint {
    type Output = ScenePoint;

    fn sub(self, rhs: ScenePoint) -> ScenePoint {
        ScenePoint {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
pub enum HeldObject {
    None,
    Sprite(Id, ScenePoint),
    Anchor(Id, i32, i32),
}

impl HeldObject {
    pub fn is_none(&self) -> bool {
        matches!(self, HeldObject::None)
    }
}

impl Default for HeldObject {
    fn default() -> HeldObject {
        HeldObject::None
    }
}

// TODO layer IDs aren't implemented

#[derive(Serialize, Deserialize)]
pub struct Layer {
    pub local_id: Id,
    pub canonical_id: Option<Id>,
    pub title: String,
    pub z: i32,
    pub sprites: Vec<Sprite>,
    pub z_min: i32,
    pub z_max: i32,
}

impl Layer {
    fn new(title: &str, z: i32) -> Self {
        Layer {
            local_id: 0,
            canonical_id: None,
            title: title.to_string(),
            z,
            sprites: Vec::new(),
            z_min: 0,
            z_max: 0,
        }
    }

    pub fn refresh_sprite_local_ids(&mut self) {
        self.sprites = self
            .sprites
            .iter_mut()
            .map(|s| Sprite::from_remote(s))
            .collect();
    }

    fn sprite(&mut self, local_id: Id) -> Option<&mut Sprite> {
        self.sprites.iter_mut().find(|s| s.local_id == local_id)
    }

    fn sprite_canonical(&mut self, canonical_id: Id) -> Option<&mut Sprite> {
        self.sprites
            .iter_mut()
            .find(|s| s.canonical_id == Some(canonical_id))
    }

    fn sort_sprites(&mut self) {
        self.sprites.sort_by(|a, b| a.z.cmp(&b.z));
    }

    fn update_z_bounds(&mut self, sprite: &Sprite) {
        if sprite.z > self.z_max {
            self.z_max = sprite.z;
        } else if sprite.z < self.z_min {
            self.z_min = sprite.z;
        }
    }

    pub fn add_sprite(&mut self, sprite: Sprite) {
        self.update_z_bounds(&sprite);
        self.sprites.push(sprite);
        self.sort_sprites();
    }

    fn add_sprites(&mut self, sprites: &mut Vec<Sprite>) {
        for s in sprites.iter() {
            self.update_z_bounds(s);
        }
        self.sprites.append(sprites);
        self.sort_sprites();
    }

    fn remove_sprite(&mut self, local_id: Id) {
        self.sprites.retain(|s| s.local_id != local_id);
    }

    fn sprite_at(&mut self, at: ScenePoint) -> Option<&mut Sprite> {
        // Reversing the iterator atm because the sprites are rendered from the
        // front of the Vec to the back, hence the last Sprite in the Vec is
        // rendered on top, and will be clicked first.
        for sprite in self.sprites.iter_mut().rev() {
            if sprite.rect.contains_point(at) {
                return Some(sprite);
            }
        }

        None
    }
}

impl Default for Layer {
    fn default() -> Self {
        Layer::new("Layer", 0)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Scene {
    // Layers of sprites in the scene. The grid is drawn at level 0.
    pub layers: Vec<Layer>,
    pub id: Option<Id>,
    pub project: Option<Id>,
    pub holding: HeldObject,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            layers: vec![
                Layer::new("Foreground", 1),
                Layer::new("Scenery", -1),
                Layer::new("Background", -2),
            ],
            id: None,
            project: None,
            holding: HeldObject::None,
        }
    }

    fn layer(&mut self, layer: Id) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.local_id == layer)
    }

    fn sprite(&mut self, local_id: Id) -> Option<&mut Sprite> {
        for layer in self.layers.iter_mut() {
            let s_opt = layer.sprite(local_id);
            if s_opt.is_some() {
                return s_opt;
            }
        }

        None
    }

    fn sprite_canonical(&mut self, canonical_id: Id) -> Option<&mut Sprite> {
        for layer in self.layers.iter_mut() {
            let s_opt = layer.sprite_canonical(canonical_id);
            if s_opt.is_some() {
                return s_opt;
            }
        }

        None
    }

    fn sprite_at(&mut self, at: ScenePoint) -> Option<&mut Sprite> {
        for layer in self.layers.iter_mut() {
            let s_opt = layer.sprite_at(at);
            if s_opt.is_some() {
                return s_opt;
            }
        }

        None
    }

    pub fn add_sprite(&mut self, sprite: Sprite, layer: Id) {
        if let Some(l) = self.layer(layer) {
            l.add_sprite(sprite);
        }
    }

    pub fn add_sprites(&mut self, mut sprites: Vec<Sprite>, layer: Id) {
        if let Some(l) = self.layer(layer) {
            l.add_sprites(&mut sprites);
        }
    }

    fn remove_sprite(&mut self, local_id: Id, layer: Id) {
        if let Some(l) = self.layer(layer) {
            l.remove_sprite(local_id);
        }
    }

    fn held_id(&self) -> Option<Id> {
        match self.holding {
            HeldObject::Sprite(id, _) => Some(id),
            HeldObject::Anchor(id, _, _) => Some(id),
            _ => None,
        }
    }

    pub fn held_sprite(&mut self) -> Option<&mut Sprite> {
        match self.held_id() {
            Some(id) => self.sprite(id),
            None => None,
        }
    }

    pub fn update_held_pos(&mut self, at: ScenePoint) -> Option<SceneEvent> {
        let holding = self.holding;
        if let Some(s) = self.held_sprite() {
            s.update_held_pos(holding, at)
        } else {
            None
        }
    }

    pub fn release_held(&mut self, snap_to_grid: bool) -> Option<SceneEvent> {
        let event = {
            if snap_to_grid {
                match self.held_sprite() {
                    Some(s) => s.snap_to_grid(),
                    None => None,
                }
            } else {
                None
            }
        };

        self.holding = HeldObject::None;
        event
    }

    pub fn grab(&mut self, at: ScenePoint) -> bool {
        match self.sprite_at(at) {
            Some(s) => {
                self.holding = s.grab(at);
                true
            }
            None => false,
        }
    }

    fn release_sprite(&mut self, canonical_id: Id) {
        if let Some(sprite) = self.held_sprite() {
            if sprite.canonical_id == Some(canonical_id) {
                self.release_held(false);
            }
        }
    }

    pub fn set_canonical_id(&mut self, local_id: Id, canonical_id: Id) {
        if let Some(s) = self.sprite(local_id) {
            s.canonical_id = Some(canonical_id);
        }
    }

    pub fn apply_event(&mut self, event: &SceneEvent, canonical: bool) -> bool {
        match *event {
            SceneEvent::SpriteNew(s, l) => {
                if let Some(canonical_id) = s.canonical_id {
                    if self.sprite_canonical(canonical_id).is_none() {
                        let sprite = Sprite::from_remote(&s);
                        self.add_sprite(sprite, l);
                        true
                    } else {
                        false
                    }
                } else {
                    let mut sprite = Sprite::from_remote(&s);
                    sprite.canonical_id = Some(sprite.local_id);
                    true
                }
            }
            SceneEvent::SpriteMove(id, from, to) => {
                self.release_sprite(id);
                match self.sprite_canonical(id) {
                    Some(s) if s.rect == from || canonical => {
                        s.set_rect(to);
                        true
                    }
                    _ => false,
                }
            }
            SceneEvent::SpriteTextureChange(id, old, new) => match self.sprite_canonical(id) {
                Some(s) if s.texture == old || canonical => {
                    s.set_texture(new);
                    true
                }
                _ => false,
            },
        }
    }

    pub fn unwind_event(&mut self, event: &SceneEvent) {
        match *event {
            SceneEvent::SpriteNew(s, l) => self.remove_sprite(s.local_id, l),
            SceneEvent::SpriteMove(id, from, to) => {
                if let Some(s) = self.sprite_canonical(id) {
                    s.set_rect(s.rect - (to - from));
                }
            }
            SceneEvent::SpriteTextureChange(id, old, _new) => {
                if let Some(s) = self.sprite_canonical(id) {
                    s.set_texture(old);
                }
            }
        }
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}
