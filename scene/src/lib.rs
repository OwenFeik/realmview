#![allow(dead_code)]
#![feature(drain_filter)]

use serde_derive::{Deserialize, Serialize};

pub mod comms;
pub mod perms;

mod layer;
mod point;
mod rect;
mod sprite;

#[cfg(test)]
mod tests;

pub use layer::Layer;
pub use point::{Point, PointVector};
pub use rect::{Dimension, Rect};
pub use sprite::{
    Cap as SpriteCap, Colour, Drawing as SpriteDrawing, Shape as SpriteShape, Sprite,
    Visual as SpriteVisual,
};

use comms::SceneEvent;

pub type Id = i64;

#[derive(Clone, Serialize, Deserialize)]
pub struct Scene {
    canon: bool,
    next_id: Id,
    pub id: Option<Id>,
    pub layers: Vec<Layer>,
    pub removed_layers: Vec<Layer>,
    pub title: Option<String>,
    pub project: Option<Id>,
    pub w: u32,
    pub h: u32,
}

impl Scene {
    const DEFAULT_SIZE: u32 = 32;

    // When creating a clone of this scene for a client, this many IDs will be
    // set aside for use by that client.
    const ID_SPACE_INCREMENT: i64 = 2_i64.pow(24);

    pub fn new() -> Self {
        Self::default()
    }

    fn next_id(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn minimise_next_id(&mut self) {
        let mut max_id = 1;
        for l in &self.layers {
            max_id = max_id.max(l.id);
            for s in &l.sprites {
                max_id = max_id.max(s.id);
            }
        }
        self.next_id = max_id + 1;
    }

    pub fn new_with_layers(layers: Vec<Layer>) -> Self {
        let mut scene = Self {
            layers,
            ..Default::default()
        };
        scene.minimise_next_id();
        scene.sort_layers();
        scene
    }

    pub fn canon(&mut self) {
        self.canon = true;
    }

    #[must_use]
    pub fn non_canon(&mut self) -> Self {
        let mut new = self.clone();
        new.canon = false;
        self.next_id += Self::ID_SPACE_INCREMENT;
        new
    }

    pub fn layer(&mut self, layer: Id) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == layer)
    }

    fn layer_ref(&self, layer: Id) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == layer)
    }

    pub fn add_layer(&mut self, layer: Layer) -> Option<SceneEvent> {
        let id = layer.id;
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

    pub fn new_layer(&mut self, title: &str, z: i32) -> Option<SceneEvent> {
        let id = self.next_id();
        self.add_layer(Layer::new(id, title, z))
    }

    pub fn remove_layer(&mut self, layer: Id) -> Option<SceneEvent> {
        let removed = self.layers.drain_filter(|l| l.id == layer).last()?;
        let event = SceneEvent::LayerRemove(removed.id);
        self.removed_layers.push(removed);
        Some(event)
    }

    fn restore_layer(&mut self, layer: Id) -> Option<SceneEvent> {
        let l = self.removed_layers.drain_filter(|l| l.id == layer).last()?;
        self.add_layer(l);
        Some(SceneEvent::LayerRestore(layer))
    }

    pub fn rename_layer(&mut self, layer: Id, new_name: String) -> Option<SceneEvent> {
        self.layer(layer).map(|l| l.rename(new_name))
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
        let i = self.layers.iter().position(|l| l.id == layer)?;

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
                Some(SceneEvent::LayerMove(self.layers[i].id, layer_z, up))
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

        let ret = Some(SceneEvent::LayerMove(self.layers[i].id, layer_z, up));
        self.sort_layers();
        ret
    }

    pub fn sprite(&mut self, id: Id) -> Option<&mut Sprite> {
        for layer in self.layers.iter_mut() {
            let s_opt = layer.sprite(id);
            if s_opt.is_some() {
                return s_opt;
            }
        }

        None
    }

    pub fn sprite_ref(&self, id: Id) -> Option<&Sprite> {
        for layer in self.layers.iter() {
            let s_opt = layer.sprite_ref(id);
            if s_opt.is_some() {
                return s_opt;
            }
        }

        None
    }

    pub fn sprite_at(&mut self, at: Point) -> Option<&mut Sprite> {
        for layer in self.layers.iter_mut() {
            // Sprites on locked or invisible layers cannot be grabbed.
            if layer.locked || !layer.visible {
                continue;
            }

            let s_opt = layer.sprite_at(at);
            if s_opt.is_some() {
                return s_opt;
            }
        }

        None
    }

    pub fn sprite_at_ref(&self, at: Point) -> Option<&Sprite> {
        for layer in &self.layers {
            if layer.locked || !layer.visible {
                continue;
            }
            let s_opt = layer.sprite_at_ref(at);
            if s_opt.is_some() {
                return s_opt;
            }
        }
        None
    }

    pub fn sprites_in(&mut self, region: Rect, all_layers: bool) -> Vec<Id> {
        let mut ids = vec![];
        for layer in &self.layers {
            if layer.selectable() {
                ids.append(&mut layer.sprites_in(region));
                if !ids.is_empty() && !all_layers {
                    return ids;
                }
            }
        }
        ids
    }

    pub fn add_sprite(&mut self, sprite: Sprite, layer: Id) -> Option<SceneEvent> {
        self.layer(layer).map(|l| l.add_sprite(sprite))
    }

    pub fn clone_sprite(&mut self, sprite: Id) -> Option<SceneEvent> {
        let l = self.get_sprite_layer(sprite)?;
        let s = self.sprite_ref(sprite)?;
        let mut new = s.clone();
        new.rect.x += new.rect.w;
        new.rect.y += new.rect.h;
        new.id = self.next_id();
        self.add_sprite(new, l)
    }

    pub fn new_sprite(&mut self, visual: Option<SpriteVisual>, layer: Id) -> Option<SceneEvent> {
        let id = self.next_id();
        self.add_sprite(Sprite::new(id, visual), layer)
    }

    pub fn add_sprites(&mut self, sprites: Vec<Sprite>, layer: Id) -> Option<SceneEvent> {
        self.layer(layer).map(|l| l.add_sprites(sprites))
    }

    pub fn remove_sprite(&mut self, id: Id) -> Option<SceneEvent> {
        for layer in &mut self.layers {
            let opt = layer.remove_sprite(id);
            if opt.is_some() {
                return opt;
            }
        }
        None
    }

    pub fn remove_sprites(&mut self, ids: &[Id]) -> SceneEvent {
        SceneEvent::EventSet(
            ids.iter()
                .filter_map(|id| self.remove_sprite(*id))
                .collect::<Vec<SceneEvent>>(),
        )
    }

    fn restore_sprite(&mut self, sprite: Id) -> Option<SceneEvent> {
        for layer in &mut self.layers {
            if layer.restore_sprite(sprite) {
                return Some(SceneEvent::SpriteRestore(sprite));
            }
        }
        None
    }

    pub fn sprite_layer(&mut self, sprite: Id, layer: Id) -> Option<SceneEvent> {
        let mut s = None;
        let mut from_id = None;
        for l in &mut self.layers {
            s = l.take_sprite(sprite);
            if s.is_some() {
                from_id = Some(l.id);
                break;
            }
        }

        let sprite = s?;
        if let Some(SceneEvent::SpriteNew(sprite, new_layer)) = self.add_sprite(sprite, layer) {
            Some(SceneEvent::SpriteLayer(sprite.id, from_id?, new_layer))
        } else {
            None
        }
    }

    pub fn sprites_layer(&mut self, sprites: &[Id], layer: Id) -> SceneEvent {
        SceneEvent::EventSet(
            sprites
                .iter()
                .filter_map(|id| self.sprite_layer(*id, layer))
                .collect::<Vec<SceneEvent>>(),
        )
    }

    fn get_sprite_layer(&self, sprite: Id) -> Option<Id> {
        self.layers
            .iter()
            .find(|l| l.sprite_ref(sprite).is_some())
            .map(|l| l.id)
    }

    pub fn event_layer(&self, event: &SceneEvent) -> Option<Id> {
        if event.is_layer() {
            event.item()
        } else if event.is_sprite() {
            self.get_sprite_layer(event.item()?)
        } else {
            None
        }
    }

    pub fn first_layer(&self) -> Id {
        self.layers.get(0).map(|l| l.id).unwrap_or(0)
    }

    pub fn apply_event(&mut self, event: SceneEvent) -> bool {
        match event {
            SceneEvent::Dummy => true,
            SceneEvent::EventSet(events) => {
                events.into_iter().map(|e| self.apply_event(e)).all(|b| b)
            }
            SceneEvent::LayerLocked(l, locked) => {
                self.layer(l).map(|l| l.set_locked(locked));
                true
            }
            SceneEvent::LayerMove(l, starting_z, up) => {
                let local_id = if let Some(layer) = self.layer(l) {
                    if layer.z != starting_z {
                        return false;
                    } else {
                        layer.id
                    }
                } else {
                    return false;
                };

                self.move_layer(local_id, up).is_some()
            }
            SceneEvent::LayerNew(id, title, z) => {
                self.add_layer(Layer::new(id, &title, z));
                true
            }
            SceneEvent::LayerRemove(l) => self.remove_layer(l).is_some(),
            SceneEvent::LayerRestore(l) => self.restore_layer(l).is_some(),
            SceneEvent::LayerRename(id, old_title, new_title) => {
                if let Some(layer) = self.layer(id) {
                    if layer.title == old_title {
                        layer.rename(new_title);
                        return true;
                    }
                }
                false
            }
            SceneEvent::LayerVisibility(l, visible) => {
                self.layer(l).map(|l| l.set_visible(visible));
                true
            }
            SceneEvent::SceneDimensions(old_w, old_h, new_w, new_h) => {
                if self.w == old_w && self.h == old_h {
                    self.w = new_w;
                    self.h = new_h;
                    true
                } else {
                    false
                }
            }
            SceneEvent::SceneTitle(old, new) => {
                if self.title == old {
                    self.title = Some(new);
                    true
                } else {
                    false
                }
            }
            SceneEvent::SpriteDrawingFinish(id) => {
                if let Some(sprite) = self.sprite(id) {
                    sprite.finish_drawing().is_some()
                } else {
                    false
                }
            }
            SceneEvent::SpriteDrawingPoint(id, n, at) => {
                if let Some(sprite) = self.sprite(id) {
                    if sprite.n_drawing_points() == n - 1 {
                        sprite.add_drawing_point(at).is_some()
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            SceneEvent::SpriteNew(s, l) => {
                if self.sprite(s.id).is_none() {
                    self.add_sprite(s, l).is_some()
                } else {
                    false
                }
            }
            SceneEvent::SpriteLayer(id, old_layer, new_layer) => {
                let old_layer_accurate = matches!(
                    self.layer_ref(old_layer).map(|l| l.sprite_ref(id)),
                    Some(Some(_))
                );

                if old_layer_accurate {
                    self.sprite_layer(id, new_layer).is_some()
                } else {
                    false
                }
            }
            SceneEvent::SpriteMove(id, from, to) => {
                let canon = self.canon;
                match self.sprite(id) {
                    Some(s) if s.rect == from || !canon => {
                        s.set_rect(to);
                        true
                    }
                    _ => false,
                }
            }
            SceneEvent::SpriteRemove(id) => {
                self.remove_sprite(id);

                // Always approve removal because the only failure mode is that
                // we didn't have that sprite in the first place, so removing
                // it is ideal.
                true
            }
            SceneEvent::SpriteRestore(id) => self.restore_sprite(id).is_some(),
            SceneEvent::SpriteVisual(id, old, new) => {
                if let Some(s) = self.sprite(id) {
                    if s.visual == old {
                        s.set_visual(new);
                        return true;
                    }
                }
                false
            }
        }
    }

    pub fn unwind_event(&mut self, event: SceneEvent) -> Option<SceneEvent> {
        match event {
            SceneEvent::Dummy => None,
            SceneEvent::EventSet(events) => Some(SceneEvent::EventSet(
                events
                    .into_iter()
                    .filter_map(|e| self.unwind_event(e))
                    .collect::<Vec<SceneEvent>>(),
            )),
            SceneEvent::LayerLocked(l, locked) => self.layer(l)?.set_locked(!locked),
            SceneEvent::LayerMove(l, _, up) => self.move_layer(l, !up),
            SceneEvent::LayerNew(id, _, _) => self.remove_layer(id),
            SceneEvent::LayerRemove(l) => self.restore_layer(l),
            SceneEvent::LayerRestore(l) => self.remove_layer(l),
            SceneEvent::LayerRename(id, old_title, _) => {
                self.layer(id).map(|l| l.rename(old_title))
            }
            SceneEvent::LayerVisibility(l, visible) => self.layer(l)?.set_visible(!visible),
            SceneEvent::SceneDimensions(old_w, old_h, new_w, new_h) => {
                if self.w == new_w && self.h == new_h {
                    self.w = old_w;
                    self.h = old_h;
                    Some(SceneEvent::SceneDimensions(new_w, new_h, old_w, old_h))
                } else {
                    None
                }
            }
            SceneEvent::SceneTitle(old, new) => {
                if self.title == Some(new.clone()) {
                    self.title = old.clone();
                    if let Some(title) = old {
                        return Some(SceneEvent::SceneTitle(Some(new), title));
                    }
                }
                None
            }
            SceneEvent::SpriteDrawingFinish(_) => None,
            SceneEvent::SpriteDrawingPoint(id, n, _) => {
                if let Some(sprite) = self.sprite(id) {
                    sprite.keep_drawing_points(n - 1);
                    sprite
                        .last_drawing_point()
                        .map(|at| SceneEvent::SpriteDrawingPoint(id, n, at))
                } else {
                    None
                }
            }
            SceneEvent::SpriteNew(s, _) => self.remove_sprite(s.id),
            SceneEvent::SpriteLayer(id, old_layer, new_layer) => {
                if self.layer_ref(new_layer)?.sprite_ref(id).is_some() {
                    self.sprite_layer(id, old_layer)
                } else {
                    None
                }
            }
            SceneEvent::SpriteMove(id, from, to) => {
                self.sprite(id).map(|s| s.set_rect(s.rect - (to - from)))
            }
            SceneEvent::SpriteRemove(id) => self.restore_sprite(id),
            SceneEvent::SpriteRestore(id) => self.remove_sprite(id),
            SceneEvent::SpriteVisual(id, old, new) => {
                let sprite = self.sprite(id)?;
                if sprite.visual == new {
                    Some(sprite.set_visual(old))
                } else {
                    None
                }
            }
        }
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            id: None,
            next_id: 4,
            canon: false,
            layers: vec![
                Layer::new(1, "Foreground", 1),
                Layer::new(2, "Scenery", -1),
                Layer::new(3, "Background", -2),
            ],
            removed_layers: vec![],
            title: None,
            project: None,
            w: Scene::DEFAULT_SIZE,
            h: Scene::DEFAULT_SIZE,
        }
    }
}
