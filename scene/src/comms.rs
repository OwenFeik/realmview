use serde_derive::{Deserialize, Serialize};

use super::{Id, ScenePoint, Sprite};

// Events processed by Scene
#[derive(Deserialize, Serialize)]
pub enum SceneEvent {
    SpriteNew(Sprite, bool),                // (new_sprite, is_token)
    SpriteMove(Id, ScenePoint, ScenePoint), // (sprite_id, from, to)
    SpriteTextureChange(Id, Id, Id),        // (sprite_id, old_texture, new_texture)
}

// Events sent by Client. The client will keep track of these after sending them
// so that it can unwind them in event of a rejection.
#[derive(Deserialize, Serialize)]
pub struct ClientEvent {
    pub id: Id,
    pub scene_event: SceneEvent,
}

// Events sent by Server. These are either an Approval / Rejection of an event
// sent by the client, or an event propagation from another client.
#[derive(Deserialize, Serialize)]
pub enum ServerEvent {
    Approval(Id),
    Rejection(Id),
    SceneChange(SceneEvent, Option<String>),
}
