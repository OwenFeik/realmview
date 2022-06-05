#![allow(dead_code)]

use serde_derive::{Deserialize, Serialize};
use std::ops::{Add, Sub};

pub mod comms;

mod layer;
mod rect;
mod sprite;

pub use layer::Layer;
pub use rect::Rect;
pub use sprite::Sprite;

use comms::{SceneEvent, SceneEventAck};

pub type Id = i64;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
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

#[derive(Clone, Serialize, Deserialize)]
pub struct Scene {
    // Layers of sprites in the scene. The grid is drawn at level 0.
    pub id: Option<Id>,
    pub layers: Vec<Layer>,
    pub title: Option<String>,
    pub project: Option<Id>,
    pub holding: HeldObject,
    pub w: u32,
    pub h: u32,
}

impl Scene {
    const DEFAULT_SIZE: u32 = 32;

    pub fn new() -> Self {
        Self {
            layers: vec![
                Layer::new("Foreground", 1),
                Layer::new("Scenery", -1),
                Layer::new("Background", -2),
            ],
            id: None,
            title: None,
            project: None,
            holding: HeldObject::None,
            w: Scene::DEFAULT_SIZE,
            h: Scene::DEFAULT_SIZE,
        }
    }

    fn layer(&mut self, layer: Id) -> Option<&mut Layer> {
        if layer == 0 {
            self.layers.iter_mut().find(|l| l.z == 1)
        } else {
            self.layers.iter_mut().find(|l| l.local_id == layer)
        }
    }

    fn layer_canonical(&mut self, layer_canonical: Id) -> Option<&mut Layer> {
        self.layers
            .iter_mut()
            .find(|l| l.canonical_id == Some(layer_canonical))
    }

    pub fn layer_canonical_ref(&self, layer_canonical: Id) -> Option<&Layer> {
        self.layers
            .iter()
            .find(|l| l.canonical_id == Some(layer_canonical))
    }

    fn add_layer(&mut self, layer: Layer) -> Option<SceneEvent> {
        let id = layer.local_id;
        if self.layer(id).is_none() {
            self.layers.push(layer);
            self.layers.sort_by(|a, b| a.z.cmp(&b.z));

            // Unwrap safe because we just pushed this.
            let layer = self.layer(id).unwrap();
            Some(SceneEvent::LayerNew(id, layer.title.clone(), layer.z))
        } else {
            None
        }
    }

    fn remove_layer(&mut self, layer: Id) {
        self.layers.retain(|l| l.local_id != layer);
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

    pub fn sprite_canonical_ref(&self, canonical_id: Id) -> Option<&Sprite> {
        for layer in self.layers.iter() {
            let s_opt = layer.sprite_canonical_ref(canonical_id);
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

    pub fn add_sprite(&mut self, sprite: Sprite, layer: Id) -> Option<SceneEvent> {
        if let Some(l) = self.layer(layer) {
            l.add_sprite(sprite);
            Some(SceneEvent::SpriteNew(sprite, l.canonical_id.unwrap_or(0)))
        } else {
            None
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
            match self.held_sprite() {
                Some(s) => {
                    if snap_to_grid {
                        s.snap_to_grid()
                    } else {
                        s.enforce_min_size()
                    }
                }
                None => None,
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

    fn set_canonical_id(&mut self, local_id: Id, canonical_id: Id) {
        if let Some(s) = self.sprite(local_id) {
            s.canonical_id = Some(canonical_id);
        }
    }

    fn set_canonical_layer_id(&mut self, local_id: Id, canonical_id: Id) {
        if let Some(l) = self.layer(local_id) {
            l.canonical_id = Some(canonical_id);
        }
    }

    // If canonical is true, this is the ground truth scene.
    pub fn apply_event(&mut self, event: SceneEvent, canonical: bool) -> SceneEventAck {
        match event {
            SceneEvent::Dummy => SceneEventAck::Approval,
            SceneEvent::LayerNew(id, title, z) => {
                let mut l = Layer::new(&title, z);

                // If this is the canonical scene, we will be taking the local
                // ID as canonical. Otherwise, the provided ID is canonical.
                if canonical {
                    l.canonical_id = Some(l.local_id);
                } else {
                    l.canonical_id = Some(id);
                }

                let canonical_id = l.canonical_id;
                self.add_layer(l);

                SceneEventAck::LayerNew(id, canonical_id)
            }
            SceneEvent::SpriteNew(s, l) => {
                if let Some(canonical_id) = s.canonical_id {
                    if self.sprite_canonical(canonical_id).is_none() {
                        let sprite = Sprite::from_remote(&s);
                        self.add_sprite(sprite, l);
                        SceneEventAck::SpriteNew(s.local_id, sprite.canonical_id)
                    } else {
                        SceneEventAck::Rejection
                    }
                } else {
                    let mut sprite = Sprite::from_remote(&s);
                    if canonical {
                        sprite.canonical_id = Some(sprite.local_id);
                    }

                    self.add_sprite(sprite, l);
                    SceneEventAck::SpriteNew(s.local_id, sprite.canonical_id)
                }
            }
            SceneEvent::SpriteMove(id, from, to) => {
                self.release_sprite(id);
                match self.sprite_canonical(id) {
                    Some(s) if s.rect == from || !canonical => {
                        s.set_rect(to);
                        SceneEventAck::Approval
                    }
                    _ => SceneEventAck::Rejection,
                }
            }
            SceneEvent::SpriteTextureChange(id, old, new) => match self.sprite_canonical(id) {
                Some(s) if s.texture == old || !canonical => {
                    s.set_texture(new);
                    SceneEventAck::Approval
                }
                _ => SceneEventAck::Rejection,
            },
        }
    }

    pub fn apply_ack(&mut self, ack: &SceneEventAck) {
        match *ack {
            SceneEventAck::SpriteNew(local_id, Some(canonical_id)) => {
                self.set_canonical_id(local_id, canonical_id);
            }
            SceneEventAck::LayerNew(local_id, Some(canonical_id)) => {
                self.set_canonical_layer_id(local_id, canonical_id);
            }
            _ => (),
        };
    }

    pub fn unwind_event(&mut self, event: &SceneEvent) {
        match *event {
            SceneEvent::Dummy => (),
            SceneEvent::LayerNew(id, _, _) => self.remove_layer(id),
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

    // Clear the local_id values from the server side, using the local id
    // pool instead to avoid conflicts.
    pub fn refresh_local_ids(&mut self) {
        for layer in &mut self.layers {
            layer.refresh_local_ids();
        }
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}
