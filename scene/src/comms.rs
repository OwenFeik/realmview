use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    perms::{Override, Perms, Role},
    Id, Point, Rect, Scene, Sprite, SpriteVisual,
};
use crate::DrawingMode;

// Events processed by Scene
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum SceneEvent {
    Dummy,                                        // To trigger redraws, etc
    EventSet(Vec<SceneEvent>),                    // Collection of other events
    FogActive(bool, bool),                        // (old, new)
    FogOcclude(bool, u32, u32),                   // (occluded, x, y)
    FogReveal(bool, u32, u32),                    // (occluded, x, y)
    GroupNew(Id),                                 // (group_id)
    GroupAdd(Id, Id),                             // (group_id, sprite_id)
    GroupRemove(Id, Id),                          // (group_id, sprite_id)
    GroupDelete(Id),                              // (group_id)
    LayerLocked(Id, bool),                        // (layer, status)
    LayerMove(Id, i32, bool),                     // (layer, starting_z, up)
    LayerNew(Id, String, i32),                    // (id, title, z, player)
    LayerRemove(Id),                              // (layer)
    LayerRename(Id, String, String),              // (layer, old_title, new_title)
    LayerRestore(Id),                             // (layer)
    LayerVisibility(Id, bool),                    // (layer, status)
    SceneDimensions(u32, u32, u32, u32),          // (old_w, old_h, new_w, new_h)
    SceneTitle(String, String),                   // (old_title, new_title)
    SpriteDrawingStart(Id, DrawingMode),          // (drawing, mode)
    SpriteDrawingPoint(Id, Point),                // (drawing, npoints, point)
    SpriteLayer(Id, Id, Id),                      // (sprite, old_layer, new_layer)
    SpriteMove(Id, Rect, Rect),                   // (sprite, from, to)
    SpriteNew(Sprite, Id),                        // (new_sprite, layer)
    SpriteRemove(Id, Id),                         // (sprite, layer)
    SpriteRestore(Id),                            // (sprite, layer)
    SpriteVisual(Id, SpriteVisual, SpriteVisual), // (sprite, old, new)
}

impl SceneEvent {
    pub fn set(events: Vec<SceneEvent>) -> Option<Self> {
        if events.is_empty() {
            None
        } else if events.len() == 1 {
            Some(events.into_iter().next().unwrap())
        } else {
            Some(Self::EventSet(events))
        }
    }

    pub fn is_fog(&self) -> bool {
        if matches!(
            self,
            Self::FogActive(..) | Self::FogOcclude(..) | Self::FogReveal(..)
        ) {
            true
        } else if let Self::EventSet(events) = self {
            events.iter().any(|e| e.is_fog())
        } else {
            false
        }
    }

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
            Self::GroupAdd(..)
                | Self::GroupRemove(..)
                | Self::SpriteDrawingPoint(..)
                | Self::SpriteLayer(..)
                | Self::SpriteMove(..)
                | Self::SpriteNew(..)
                | Self::SpriteRemove(..)
                | Self::SpriteRestore(..)
                | Self::SpriteVisual(..)
        ) {
            true
        } else if let Self::EventSet(events) = self {
            events.iter().any(Self::is_sprite)
        } else {
            false
        }
    }

    pub fn is_scene(&self) -> bool {
        if matches!(
            self,
            Self::SceneDimensions(..) | Self::SceneTitle(..) | Self::FogActive(..)
        ) {
            true
        } else if let Self::EventSet(events) = self {
            events.iter().any(Self::is_scene)
        } else {
            false
        }
    }

    // If is_sprite or is_layer is true, this will be safe to unwrap.
    pub fn item(&self) -> Option<Id> {
        match self {
            &Self::GroupAdd(_, id)
            | &Self::GroupRemove(_, id)
            | &Self::LayerLocked(id, ..)
            | &Self::LayerMove(id, ..)
            | &Self::LayerNew(id, ..)
            | &Self::LayerRemove(id)
            | &Self::LayerRename(id, ..)
            | &Self::LayerRestore(id)
            | &Self::LayerVisibility(id, ..)
            | &Self::SpriteLayer(id, ..)
            | &Self::SpriteMove(id, ..)
            | &Self::SpriteRemove(id, ..)
            | &Self::SpriteRestore(id)
            | &Self::SpriteVisual(id, ..)
            | &Self::SpriteDrawingStart(id, ..)
            | &Self::SpriteDrawingPoint(id, ..) => Some(id),
            Self::SpriteNew(s, ..) => Some(s.id),
            Self::Dummy
            | Self::EventSet(_)
            | Self::FogActive(_, _)
            | Self::FogOcclude(_, _, _)
            | Self::FogReveal(_, _, _)
            | Self::GroupNew(_)
            | Self::GroupDelete(_)
            | Self::SceneDimensions(_, _, _, _)
            | Self::SceneTitle(_, _) => None,
        }
    }

    pub fn sprite(&self) -> Option<Id> {
        Some(match self {
            &Self::GroupAdd(_, id) => id,
            &Self::GroupRemove(_, id) => id,
            &Self::SpriteLayer(id, ..) => id,
            &Self::SpriteMove(id, ..) => id,
            Self::SpriteNew(s, ..) => s.id,
            &Self::SpriteRemove(id, ..) => id,
            &Self::SpriteRestore(id) => id,
            &Self::SpriteVisual(id, ..) => id,
            _ => return None,
        })
    }

    pub fn layer(&self) -> Option<Id> {
        Some(match *self {
            Self::LayerLocked(id, ..) => id,
            Self::LayerMove(id, ..) => id,
            Self::LayerNew(id, ..) => id,
            Self::LayerRename(id, ..) => id,
            Self::LayerRestore(id) => id,
            Self::LayerVisibility(id, ..) => id,
            Self::SpriteLayer(.., layer) => layer,
            Self::SpriteNew(.., layer) => layer,
            Self::SpriteRemove(.., layer) => layer,
            _ => return None,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum PermsEvent {
    /// Update to the role of a user
    RoleChange(Uuid, Role),
    /// Issue a new Override
    NewOverride(Override),
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ClientEvent {
    Ping,
    SceneUpdate(SceneEvent), // (event)
    SceneChange(Uuid),       // (scene_uuid)
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
    EventSet(Vec<ServerEvent>),
    GameOver,
    Disconnect,
    HealthCheck,
    Rejection(Id),
    PermsChange(Perms),
    PermsUpdate(PermsEvent),
    SceneChange(Box<Scene>),
    SceneList(Vec<(String, Uuid)>, Uuid),
    SceneUpdate(SceneEvent),
    SelectedLayer(Id),
    UserId(Uuid),
}

impl ServerEvent {
    pub fn set(events: Vec<ServerEvent>) -> Option<ServerEvent> {
        if events.is_empty() {
            None
        } else if events.len() == 1 {
            Some(events.into_iter().next().unwrap())
        } else {
            Some(Self::EventSet(events))
        }
    }
}
