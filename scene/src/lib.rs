#![allow(dead_code)]
#![feature(drain_filter)]

use serde_derive::{Deserialize, Serialize};
use std::ops::{Add, Sub};

pub mod comms;

mod layer;
mod rect;
mod sprite;

#[cfg(test)]
mod tests;

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

    // Return the rectangle formed by these two points.
    pub fn rect(&self, ScenePoint { x, y }: ScenePoint) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            w: x - self.x,
            h: y - self.y,
        }
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

#[derive(Clone, Serialize, Deserialize)]
pub struct Scene {
    canon: bool,
    pub id: Option<Id>,
    pub layers: Vec<Layer>,
    pub removed_layers: Vec<Layer>,
    pub sprites: Vec<Sprite>,
    pub removed_sprites: Vec<Sprite>,
    pub title: Option<String>,
    pub project: Option<Id>,
    pub w: u32,
    pub h: u32,
}

impl Scene {
    const DEFAULT_SIZE: u32 = 32;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_layers(layers: Vec<Layer>) -> Self {
        let mut scene = Self {
            layers,
            ..Default::default()
        };
        scene.sort_layers();
        scene
    }

    /// Updates this scene to be the canonical Scene for a game.
    pub fn canon(&mut self) {
        for layer in &mut self.layers {
            if layer.canonical_id.is_none() {
                layer.canonical_id = Some(layer.local_id);
            }
        }

        for sprite in &mut self.sprites {
            if sprite.canonical_id.is_none() {
                sprite.canonical_id = Some(sprite.local_id);
            }
        }

        self.canon = true;
    }

    /// Returns a non-canonical clone of this Scene.
    #[must_use]
    pub fn non_canon(&self) -> Self {
        let mut new = self.clone();
        new.canon = false;
        new
    }

    /// Returns the layer with the given local_id if it exists.
    pub fn layer(&mut self, local_id: Id) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.local_id == local_id)
    }

    fn layer_ref(&self, local_id: Id) -> Option<&Layer> {
        self.layers.iter().find(|l| l.local_id == local_id)
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

    fn layer_local(&self, canonical_id: Id) -> Option<Id> {
        self.layer_canonical_ref(canonical_id)
            .map(|l| l.local_id)
    }

    pub fn add_layer(&mut self, layer: Layer) -> Option<SceneEvent> {
        let id = layer.local_id;
        if self.layer(id).is_none() {
            self.layers.push(layer);
            self.sort_layers();

            // Unwrap safe because we just pushed this.
            let layer = self.layer(id).unwrap();
            Some(SceneEvent::LayerNew(id, layer.title.clone(), layer.z))
        } else {
            None
        }
    }

    pub fn remove_layer(&mut self, layer: Id) -> Option<SceneEvent> {
        let removed = self.layers.drain_filter(|l| l.local_id == layer).last()?;
        let event = removed.canonical_id.map(SceneEvent::LayerRemove);

        // If this removal might be rejected, we'll keep the layer around to
        // restore.
        if event.is_some() {
            self.removed_layers.push(removed);
        }
        event
    }

    fn restore_layer(&mut self, layer_canonical: Id) {
        if let Some(layer) = self
            .removed_layers
            .drain_filter(|l| l.canonical_id == Some(layer_canonical))
            .last()
        {
            self.add_layer(layer);
        }
    }

    fn remove_layer_canonical(&mut self, layer: Id) -> Option<SceneEvent> {
        let local_id = self.layer_canonical(layer)?.local_id;
        self.remove_layer(local_id)
    }

    pub fn rename_layer(&mut self, layer: Id, new_name: String) -> Option<SceneEvent> {
        if let Some(l) = self.layer(layer) {
            l.rename(new_name)
        } else {
            None
        }
    }

    // Sort to place the highest layer first. Also updates layer z values to
    // simplify.
    pub fn sort_layers(&mut self) {
        self.layers.sort_by(|a, b| b.z.cmp(&a.z));

        // Use the smallest range of z values possible, to ensure a consistent set
        // of zs across clients.
        if let Some(i) = self.layers.iter().position(|l| l.z < 0) {
            let mut z = i as i32;
            for layer in &mut self.layers[..i] {
                layer.z = z;
                z -= 1;
            }

            let mut z = -1;
            for layer in &mut self.layers[i..] {
                layer.z = z;
                z -= 1;
            }
        } else {
            let mut z = self.layers.len() as i32;
            for layer in &mut self.layers {
                layer.z = z;
                z -= 1;
            }
        }
    }

    pub fn move_layer(&mut self, layer: Id, up: bool) -> Option<SceneEvent> {
        let i = self.layers.iter().position(|l| l.local_id == layer)?;

        // Get layer height. Safe to unwrap as we just found this index with
        // position.
        let layer_z = self.layers.get(i).unwrap().z;

        let down = !up;
        if (up && i == 0) || (down && i == self.layers.len() - 1) {
            // This layer is already at an extreme of the layer stack.
            // If this is the top layer and in the background or the bottom
            // layer and in the foreground, move it to the other side.
            // Otherwise do nothing.
            return if (up && layer_z < 0) || (down && layer_z > 0) {
                self.layers[i].z = if up { 1 } else { -1 };
                self.sort_layers();
                self.layers[i]
                    .canonical_id
                    .map(|id| SceneEvent::LayerMove(id, layer_z, up))
            } else {
                None
            };
        }

        // Get height of layer above. This unwrap is safe as we know that
        // the index of layer is greater than 0 so there must be an element
        // at i - 1.
        let other_i = if up { i - 1 } else { i + 1 };
        let other_z = self.layers.get_mut(other_i).unwrap().z;
        if layer_z.signum() == other_z.signum() {
            // If these layers are on the same side of the grid, we can just
            // swap their z values.
            self.layers[i].z = other_z;
            self.layers[other_i].z = layer_z;
        } else if up {
            // We now know that it must be that case that we are moving this
            // layer up past the grid, so increase z of all layers above
            // background, set layer z to 1. i must be the index of the first
            // layer below the grid.
            for layer in &mut self.layers[0..=other_i] {
                layer.z += 1;
            }
            self.layers[i].z = 1;
        } else {
            // We now know that it must be that case that we are moving this
            // layer down past the grid, so decrease z of all layers below
            // background, set layer z to -1.
            for layer in &mut self.layers[other_i..] {
                layer.z -= 1;
            }
            self.layers[i].z = -1;
        }

        let ret = self.layers[i]
            .canonical_id
            .map(|id| SceneEvent::LayerMove(id, layer_z, up));
        self.sort_layers();
        ret
    }

    fn sprite(&mut self, local_id: Id) -> Option<&mut Sprite> {
        self.sprites.iter_mut().find(|s| s.local_id == local_id)
    }

    fn sprite_canonical(&mut self, canonical_id: Id) -> Option<&mut Sprite> {
        self.sprites.iter_mut().find(|s| s.canonical_id == Some(canonical_id))
    }

    fn sprite_canonical_ref(&self, canonical_id: Id) -> Option<&Sprite> {
        self.sprites.iter().find(|s| s.canonical_id == Some(canonical_id))
    }

    fn sprite_local(&self, canonical_id: Id) -> Option<Id> {
        self.sprites.iter().find(|l| l.canonical_id == Some(canonical_id)).map(|s| s.local_id)
    }

    fn add_sprite(&self, sprite: Sprite) {
        self.sprites.push(sprite)
    }

    fn restore_sprite(&mut self, canonical_id: Id) {
        if let Some(s) = self
            .removed_sprites
            .drain_filter(|s| s.canonical_id == Some(canonical_id))
            .last()
        {
            self.sprites.push(s);
        }
    }

    fn take_sprite(&mut self, local_id: Id) -> Option<Sprite> {
        self.sprites.drain_filter(|s| s.local_id == local_id).last()
    }

    fn take_sprite_canonical(&mut self, canonical_id: Id) -> Option<Sprite> {
        self.sprites
            .drain_filter(|s| s.canonical_id == Some(canonical_id))
            .last()
    }

    pub fn remove_sprite(&mut self, local_id: Id) -> Option<SceneEvent> {
        if let Some(s) = self.take_sprite(local_id) {
            self.removed_sprites.push(s);
            s.canonical_id.map(SceneEvent::SpriteRemove)
        } else {
            None
        }
    }


    // TODO order sprites
    pub fn sprite_at(&mut self, at: ScenePoint) -> Option<&mut Sprite> {
        for sprite in self.sprites.iter_mut().rev() {
            if sprite.rect.contains_point(at) {
                return Some(sprite);
            }
        }

        None
    }

    pub fn sprite_at_ref(&self, at: ScenePoint) -> Option<&Sprite> {
        for sprite in self.sprites.iter().rev() {
            if sprite.rect.contains_point(at) {
                return Some(sprite);
            }
        }

        None
    }

    // TODO single / multiple layer
    pub fn sprites_in(&self, region: Rect) -> Vec<Id> {
        let mut ret = vec![];
        for sprite in &self.sprites {
            if region.contains_rect(sprite.rect) {
                ret.push(sprite.local_id);
            }
        }
        ret
    }

    pub fn sprite_layer(&mut self, sprite_lid: Id, layer_lid: Id) -> Option<SceneEvent> {
        if let Some(layer) = self.layer_ref(layer_lid) {
            if let Some(sprite) = self.sprite(sprite_lid) {
                let old_layer_lid = sprite.layer;
                sprite.layer = layer_lid;

                // Only emit an event if we have canonical IDs for the sprite,
                // the old layer and the new layer.
                if let Some(sprite_id) = sprite.canonical_id {
                    if let Some(new_layer) = layer.canonical_id {
                        if let Some(Some(old_layer)) = self.layer_ref(old_layer_lid).map(|l| l.canonical_id) {
                            return sprite.canonical_id.map(|sprite_id| SceneEvent::SpriteLayer(sprite_id, old_layer, new_layer));
                        }
                    }
                }
            }
        }
        
        None
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
    pub fn apply_event(&mut self, event: SceneEvent) -> SceneEventAck {
        match event {
            SceneEvent::Dummy => SceneEventAck::Approval,
            SceneEvent::LayerLocked(l, locked) => {
                self.layer_canonical(l).map(|l| l.set_locked(locked));
                SceneEventAck::Approval
            }
            SceneEvent::LayerMove(l, starting_z, up) => {
                let local_id = if let Some(layer) = self.layer_canonical(l) {
                    if layer.z != starting_z {
                        return SceneEventAck::Rejection;
                    } else {
                        layer.local_id
                    }
                } else {
                    return SceneEventAck::Rejection;
                };

                SceneEventAck::from(self.move_layer(local_id, up).is_some())
            }
            SceneEvent::LayerNew(id, title, z) => {
                let mut l = Layer::new(&title, z);

                // If this is the canonical scene, we will be taking the local
                // ID as canonical. Otherwise, the provided ID is canonical.
                if self.canon {
                    l.canonical_id = Some(l.local_id);
                } else {
                    l.canonical_id = Some(id);
                }

                let canonical_id = l.canonical_id;
                self.add_layer(l);

                SceneEventAck::LayerNew(id, canonical_id)
            }
            SceneEvent::LayerRemove(l) => {
                SceneEventAck::from(self.remove_layer_canonical(l).is_some())
            }
            SceneEvent::LayerRename(id, old_title, new_title) => {
                if let Some(layer) = self.layer_canonical(id) {
                    if layer.title == old_title {
                        layer.rename(new_title);
                        SceneEventAck::Approval
                    } else {
                        SceneEventAck::Rejection
                    }
                } else {
                    SceneEventAck::Rejection
                }
            }
            SceneEvent::LayerVisibility(l, visible) => {
                self.layer_canonical(l).map(|l| l.set_visible(visible));
                SceneEventAck::Approval
            }
            SceneEvent::SpriteNew(s, l) => {
                if let Some(sprite_cid) = s.canonical_id {
                    if self.sprite_canonical(sprite_cid).is_none() {
                        if let Some(layer_lid) = self.layer_local(l) {
                            let sprite = Sprite::from_remote(&s, layer_lid);
                            self.add_sprite(sprite);
                            return SceneEventAck::SpriteNew(s.local_id, sprite.canonical_id);
                        }
                    }
                } else if let Some(layer_lid) = self.layer_local(l) {
                    let mut sprite = Sprite::from_remote(&s, layer_lid);
                    if self.canon {
                        sprite.canonical_id = Some(sprite.local_id);
                    }
                    self.add_sprite(sprite);
                    return SceneEventAck::SpriteNew(sprite.local_id, sprite.canonical_id);
                }
                SceneEventAck::Rejection
            }
            SceneEvent::SpriteLayer(id, old_layer, new_layer) => {
                if let (Some(old_lid), Some(new_lid), Some(s)) = (self.layer_local(old_layer), self.layer_local(new_layer), self.sprite_canonical(id)) {
                    if s.layer == old_lid {
                        s.layer = new_lid;
                        return SceneEventAck::Approval
                    }
                };
                SceneEventAck::Rejection
            }
            SceneEvent::SpriteMove(id, from, to) => {
                let canon = self.canon;
                match self.sprite_canonical(id) {
                    Some(s) if s.rect == from || !canon => {
                        s.set_rect(to);
                        SceneEventAck::Approval
                    }
                    _ => SceneEventAck::Rejection,
                }
            }
            SceneEvent::SpriteRemove(id) => {
                let local_id = match self.sprite_canonical_ref(id) {
                    Some(s) => s.local_id,
                    _ => return SceneEventAck::Rejection,
                };
                self.remove_sprite(local_id);
                SceneEventAck::Approval
            }
            SceneEvent::SpriteTexture(id, old, new) => {
                let canon = !self.canon;
                match self.sprite_canonical(id) {
                    Some(s) if s.texture == old || !canon => {
                        s.set_texture(new);
                        SceneEventAck::Approval
                    }
                    _ => SceneEventAck::Rejection,
                }
            }
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

    pub fn unwind_event(&mut self, event: SceneEvent) {
        match event {
            SceneEvent::Dummy => (),
            SceneEvent::LayerLocked(l, locked) => {
                self.layer_canonical(l).map(|l| l.set_locked(!locked));
            }
            SceneEvent::LayerMove(l, _, up) => {
                let local_id = if let Some(layer) = self.layer_canonical(l) {
                    layer.local_id
                } else {
                    return;
                };

                self.move_layer(local_id, !up);
            }
            SceneEvent::LayerNew(id, _, _) => {
                self.remove_layer(id);
            }
            SceneEvent::LayerRemove(l) => self.restore_layer(l),
            SceneEvent::LayerRename(id, old_title, _) => {
                if let Some(l) = self.layer_canonical(id) {
                    l.rename(old_title);
                }
            }
            SceneEvent::LayerVisibility(l, visible) => {
                self.layer_canonical(l).map(|l| l.set_visible(!visible));
            }
            SceneEvent::SpriteNew(s, _) => {
                self.remove_sprite(s.local_id);
            }
            SceneEvent::SpriteLayer(id, old_layer, new_layer) => {
                let sprite = if let Some(l) = self.layer_canonical(new_layer) {
                    if let Some(s) = l.take_sprite_canonical(id) {
                        s
                    } else {
                        return;
                    }
                } else {
                    return;
                };

                if let Some(layer) = self.layer_canonical(old_layer) {
                    layer.add_sprite(sprite);
                }
            }
            SceneEvent::SpriteMove(id, from, to) => {
                if let Some(s) = self.sprite_canonical(id) {
                    s.set_rect(s.rect - (to - from));
                }
            }
            SceneEvent::SpriteRemove(id) => {
                self.restore_sprite(id);
            }
            SceneEvent::SpriteTexture(id, old, _new) => {
                if let Some(s) = self.sprite_canonical(id) {
                    s.set_texture(old);
                }
            }
        }
    }

    /// When using a serialised scene from the remote server, this should be
    /// called to use the local ID pool instead.
    pub fn refresh_local_ids(&mut self) {
        for layer in &mut self.layers {
            layer.refresh_local_id();
        }

        for sprite in &mut self.sprites {
            sprite.refresh_local_id();
        }
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            id: None,
            canon: false,
            layers: vec![
                Layer::new("Foreground", 1),
                Layer::new("Scenery", -1),
                Layer::new("Background", -2),
            ],
            removed_layers: vec![],
            sprites: vec![],
            removed_sprites: vec![],
            title: None,
            project: None,
            w: Scene::DEFAULT_SIZE,
            h: Scene::DEFAULT_SIZE,
        }
    }
}
