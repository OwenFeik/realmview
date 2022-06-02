use serde_derive::{Deserialize, Serialize};

use super::{Id, Rect, Sprite};

// Events processed by Scene
#[derive(Deserialize, Serialize)]
pub enum SceneEvent {
    Dummy,
    SpriteNew(Sprite, Id),           // (new_sprite, layer)
    SpriteMove(Id, Rect, Rect),      // (sprite_id, from, to)
    SpriteTextureChange(Id, Id, Id), // (sprite_id, old_texture, new_texture)
}

#[derive(Deserialize, Serialize)]
pub enum ClientEvent {
    Ping,
    SceneChange(SceneEvent),
}

// Events sent by Client. The client will keep track of these after sending them
// so that it can unwind them in event of a rejection.
#[derive(Deserialize, Serialize)]
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
    SceneChange(SceneEvent, Option<String>),
    CanonicalId(Id, Id),
}
