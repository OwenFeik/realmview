use serde_derive::{Deserialize, Serialize};

use super::{Id, Rect, Scene, Sprite};

// Events processed by Scene
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum SceneEvent {
    Dummy,                           // To trigger redraws, etc
    LayerLockedChange(Id, bool),     // (layer, status)
    LayerMove(Id, i32, bool),        // (layer, starting_z, up)
    LayerNew(Id, String, i32),       // (local_id, title, z)
    LayerRemove(Id),                 // (layer)
    LayerRename(Id, String, String), // (layer, old_title, new_title)
    LayerVisibilityChange(Id, bool), // (layer, status)
    SpriteNew(Sprite, Id),           // (new_sprite, layer)
    SpriteMove(Id, Rect, Rect),      // (sprite_id, from, to)
    SpriteTextureChange(Id, Id, Id), // (sprite_id, old_texture, new_texture)
}

impl SceneEvent {
    pub fn is_layer(&self) -> bool {
        matches!(
            self,
            Self::LayerLockedChange(..)
                | Self::LayerMove(..)
                | Self::LayerNew(..)
                | Self::LayerRemove(..)
                | Self::LayerRename(..)
                | Self::LayerVisibilityChange(..)
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum SceneEventAck {
    Approval,                  // Catchall OK
    Rejection,                 // Catchall reject
    LayerNew(Id, Option<Id>),  // (original_id, canonical_id)
    SpriteNew(Id, Option<Id>), // (original_id, canonical_id)
}

impl SceneEventAck {
    pub fn from(approved: bool) -> Self {
        if approved {
            Self::Approval
        } else {
            Self::Rejection
        }
    }

    pub fn is_approval(&self) -> bool {
        matches!(
            self, 
            Self::Approval
                | Self::LayerNew(..)
                | Self::SpriteNew(..) 
        )
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
    Ack(Id, Option<SceneEventAck>),
    SceneChange(Scene),
    SceneUpdate(SceneEvent),
}
