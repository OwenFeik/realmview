use std::sync::atomic::{AtomicI64, Ordering};

use serde_derive::{Deserialize, Serialize};

use crate::comms::SceneEvent;

use super::Id;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layer {
    pub local_id: Id,
    pub canonical_id: Option<Id>,
    pub title: String,
    pub z: i32,
    pub visible: bool,
    pub locked: bool,
}

impl Layer {
    fn next_id() -> Id {
        static LAYER_ID: AtomicI64 = AtomicI64::new(1);
        LAYER_ID.fetch_add(1, Ordering::Relaxed)
    }

    pub fn new(title: &str, z: i32) -> Self {
        Layer {
            local_id: Self::next_id(),
            canonical_id: None,
            title: title.to_string(),
            z,
            visible: true,
            locked: false,
        }
    }

    pub fn refresh_local_id(&mut self) {
        self.local_id = Self::next_id();
    }

    pub fn rename(&mut self, new_title: String) -> Option<SceneEvent> {
        let mut old_title = new_title;
        std::mem::swap(&mut old_title, &mut self.title);
        self.canonical_id
            .map(|id| SceneEvent::LayerRename(id, old_title, self.title.clone()))
    }

    pub fn set_visible(&mut self, visible: bool) -> Option<SceneEvent> {
        if self.visible != visible {
            self.visible = visible;
            self.canonical_id
                .map(|id| SceneEvent::LayerVisibility(id, visible))
        } else {
            None
        }
    }

    pub fn set_locked(&mut self, locked: bool) -> Option<SceneEvent> {
        if self.locked != locked {
            self.locked = locked;
            self.canonical_id
                .map(|id| SceneEvent::LayerLocked(id, locked))
        } else {
            None
        }
    }

    // Sprites can only be selected from a layer if it is both visible and
    // unlocked.
    pub fn selectable(&self) -> bool {
        self.visible && !self.locked
    }
}

impl Default for Layer {
    fn default() -> Self {
        Layer::new("Layer", 0)
    }
}
