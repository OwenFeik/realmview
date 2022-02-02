use serde_derive::{Deserialize, Serialize};

use super::{Id, ScenePoint, Sprite};

// texture_url fields are optional because they are only used in server to
// client messages as the server already knows the texture URLs (and doesn't
// need them anyway).

#[derive(Deserialize, Serialize)]
pub enum SceneEvent {
    SpriteNew(Sprite, bool, Option<String>), // (new_sprite, is_token, texture_url)
    SpriteMove(Id, ScenePoint, ScenePoint),  // (sprite_id, from, to)
    SpriteTextureChange(Id, Id, Id, Option<String>), // (sprite_id, old_texture, new_texture, texture_url)
}
