use serde_derive::{Deserialize, Serialize};

use super::{Id, Rect, Scene, Sprite};

// Events processed by Scene
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum SceneEvent {
    Dummy,                           // To trigger redraws, etc
    EventSet(Vec<SceneEvent>),       // Collection of other events
    LayerLocked(Id, bool),           // (layer, status)
    LayerMove(Id, i32, bool),        // (layer, starting_z, up)
    LayerNew(Id, String, i32),       // (local_id, title, z)
    LayerRemove(Id),                 // (layer)
    LayerRename(Id, String, String), // (layer, old_title, new_title)
    LayerRestore(Id),                // (layer)
    LayerVisibility(Id, bool),       // (layer, status)
    SpriteLayer(Id, Id, Id),         // (sprite, old_layer, new_layer)
    SpriteMove(Id, Rect, Rect),      // (sprite, from, to)
    SpriteNew(Sprite, Id),           // (new_sprite, layer)
    SpriteRemove(Id),                // (sprite)
    SpriteRestore(Id),               // (sprite)
    SpriteTexture(Id, Id, Id),       // (sprite, old_texture, new_texture)
}

impl SceneEvent {
    pub fn is_layer(&self) -> bool {
        matches!(
            self,
            Self::LayerLocked(..)
                | Self::LayerMove(..)
                | Self::LayerNew(..)
                | Self::LayerRemove(..)
                | Self::LayerRename(..)
                | Self::LayerRestore(..)
                | Self::LayerVisibility(..)
        )
    }

    pub fn is_sprite(&self) -> bool {
        matches!(
            self,
            Self::SpriteLayer(..)
                | Self::SpriteMove(..)
                | Self::SpriteNew(..)
                | Self::SpriteRemove(..)
                | Self::SpriteRestore(..)
                | Self::SpriteTexture(..)
        )
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
            Self::SpriteTexture(id, ..) => id,
            _ => return None,
        };
        Some(*id)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ClientEvent {
    Ping,
    SceneChange(SceneEvent),
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
    SceneChange(Scene),
    SceneUpdate(SceneEvent),
}
