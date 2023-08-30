use serde_derive::{Deserialize, Serialize};

use super::{Id, Sprite};
use crate::{comms::SceneEvent, Rect};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layer {
    pub id: Id,
    pub title: String,
    pub z: i32,
    pub visible: bool,
    pub locked: bool,
    pub sprites: Vec<Sprite>,
    pub removed_sprites: Vec<Sprite>,
    pub z_min: i32,
    pub z_max: i32,
}

impl Layer {
    pub fn new(id: i64, title: &str, z: i32) -> Self {
        Layer {
            id,
            title: title.to_string(),
            z,
            visible: true,
            locked: false,
            sprites: vec![],
            removed_sprites: vec![],
            z_min: 0,
            z_max: 0,
        }
    }

    pub fn rename(&mut self, new_title: String) -> SceneEvent {
        let mut old_title = new_title;
        std::mem::swap(&mut old_title, &mut self.title);
        SceneEvent::LayerRename(self.id, old_title, self.title.clone())
    }

    pub fn set_visible(&mut self, visible: bool) -> Option<SceneEvent> {
        if self.visible != visible {
            self.visible = visible;
            Some(SceneEvent::LayerVisibility(self.id, visible))
        } else {
            None
        }
    }

    pub fn set_locked(&mut self, locked: bool) -> Option<SceneEvent> {
        if self.locked != locked {
            self.locked = locked;
            Some(SceneEvent::LayerLocked(self.id, locked))
        } else {
            None
        }
    }

    // Sprites can only be selected from a layer if it is both visible and
    // unlocked.
    pub fn selectable(&self) -> bool {
        self.visible && !self.locked
    }

    pub fn sprite(&mut self, id: Id) -> Option<&mut Sprite> {
        self.sprites.iter_mut().find(|s| s.id == id)
    }

    pub fn sprite_ref(&self, id: Id) -> Option<&Sprite> {
        self.sprites.iter().find(|s| s.id == id)
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

    pub fn add_sprite(&mut self, sprite: Sprite) -> SceneEvent {
        self.update_z_bounds(&sprite);
        self.sprites.push(sprite.clone());
        self.sort_sprites();
        SceneEvent::SpriteNew(sprite, self.id)
    }

    pub fn add_sprites(&mut self, sprites: Vec<Sprite>) -> SceneEvent {
        SceneEvent::EventSet(sprites.into_iter().map(|s| self.add_sprite(s)).collect())
    }

    pub fn restore_sprite(&mut self, id: Id) -> bool {
        if let Some(s) = self.removed_sprites.extract_if(|s| s.id == id).last() {
            self.add_sprite(s);
            true
        } else {
            false
        }
    }

    pub fn take_sprite(&mut self, id: Id) -> Option<Sprite> {
        self.sprites.extract_if(|s| s.id == id).last()
    }

    pub fn remove_sprite(&mut self, id: Id) -> Option<SceneEvent> {
        if let Some(s) = self.take_sprite(id) {
            self.removed_sprites.push(s);
            Some(SceneEvent::SpriteRemove(id))
        } else {
            None
        }
    }

    pub fn sprites_in(&self, region: Rect) -> Vec<Id> {
        let mut ret = vec![];
        for sprite in &self.sprites {
            if region.contains_rect(sprite.rect) {
                ret.push(sprite.id);
            }
        }
        ret
    }
}
