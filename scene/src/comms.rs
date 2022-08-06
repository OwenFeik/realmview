use serde_derive::{Deserialize, Serialize};

use super::{
    perms::{Override, PermSet, Perms, Role},
    Id, Rect, Scene, Sprite, SpriteVisual,
};

// Events processed by Scene
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum SceneEvent {
    Dummy,                                        // To trigger redraws, etc
    EventSet(Vec<SceneEvent>),                    // Collection of other events
    LayerLocked(Id, bool),                        // (layer, status)
    LayerMove(Id, i32, bool),                     // (layer, starting_z, up)
    LayerNew(Id, String, i32),                    // (local_id, title, z)
    LayerRemove(Id),                              // (layer)
    LayerRename(Id, String, String),              // (layer, old_title, new_title)
    LayerRestore(Id),                             // (layer)
    LayerVisibility(Id, bool),                    // (layer, status)
    SceneDimensions(u32, u32, u32, u32),          // (old_w, old_h, new_w, new_h)
    SceneTitle(Option<String>, String),           // (old_title, new_title)
    SpriteLayer(Id, Id, Id),                      // (sprite, old_layer, new_layer)
    SpriteMove(Id, Rect, Rect),                   // (sprite, from, to)
    SpriteNew(Sprite, Id),                        // (new_sprite, layer)
    SpriteRemove(Id),                             // (sprite)
    SpriteRestore(Id),                            // (sprite)
    SpriteVisual(Id, SpriteVisual, SpriteVisual), // (sprite, old, new)
}

impl SceneEvent {
    pub fn is_layer(&self) -> bool {
        if matches!(
            self,
            Self::LayerLocked(..)
                | Self::LayerMove(..)
                | Self::LayerNew(..)
                | Self::LayerRemove(..)
                | Self::LayerRename(..)
                | Self::LayerRestore(..)
                | Self::LayerVisibility(..)
        ) {
            true
        } else if let Self::EventSet(events) = self {
            events.iter().any(|e| e.is_layer())
        } else {
            false
        }
    }

    pub fn is_sprite(&self) -> bool {
        if matches!(
            self,
            Self::SpriteLayer(..)
                | Self::SpriteMove(..)
                | Self::SpriteNew(..)
                | Self::SpriteRemove(..)
                | Self::SpriteRestore(..)
                | Self::SpriteVisual(..)
        ) {
            true
        } else if let Self::EventSet(events) = self {
            events.iter().any(|e| e.is_sprite())
        } else {
            false
        }
    }

    // If is_sprite or is_layer is true, this will be safe to unwrap.
    pub fn item(&self) -> Option<Id> {
        let id = match self {
            Self::LayerLocked(id, ..) => id,
            Self::LayerMove(id, ..) => id,
            Self::LayerNew(id, ..) => id,
            Self::LayerRename(id, ..) => id,
            Self::LayerRestore(id) => id,
            Self::LayerVisibility(id, ..) => id,
            Self::SpriteLayer(id, ..) => id,
            Self::SpriteMove(id, ..) => id,
            Self::SpriteNew(s, ..) => &s.id,
            Self::SpriteRemove(id) => id,
            Self::SpriteRestore(id) => id,
            Self::SpriteVisual(id, ..) => id,
            _ => return None,
        };
        Some(*id)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum PermsEvent {
    /// Update to the role of a user
    RoleChange(Id, Role),
    /// Replace the PermSet for an item
    ItemPerms(PermSet),
    /// Issue a new Override
    NewOverride(Override),
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ClientEvent {
    Ping,
    SceneUpdate(SceneEvent),
}

// Events sent by Client. The client will keep track of these after sending them
// so that it can unwind them in event of a rejection.
#[derive(Debug, Deserialize, Serialize)]
pub struct ClientMessage {
    pub id: Id,
    pub event: ClientEvent,
}

// Events sent by Server. These are either an Approval / Rejection of an event
// sent by the client, or an event propagation from another client.
#[derive(Deserialize, Serialize)]
pub enum ServerEvent {
    Approval(Id),
    Rejection(Id),
    PermsChange(Perms),
    PermsUpdate(PermsEvent),
    SceneChange(Scene),
    SceneUpdate(SceneEvent),
    UserId(Id),
}
