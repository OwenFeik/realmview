use std::collections::HashMap;

use scene::Id;

use crate::{
    scene::{
        comms::{PermsEvent, SceneEvent},
        perms::{self, Perms},
        Scene,
    },
    utils::warning,
};

pub struct Game {
    pub key: String,
    pub project: i64,

    scene: Scene,
    perms: Perms,
    users: HashMap<i64, String>,
}

impl Game {
    pub fn new(project: i64, mut scene: Scene, owner: i64, key: &str) -> Self {
        scene.canon();
        let mut perms = Perms::new();
        perms.set_owner(owner);
        Self {
            project,
            key: key.to_owned(),
            scene,
            perms,
            users: HashMap::new(),
        }
    }

    pub fn project_id(&self) -> Option<i64> {
        self.scene.project
    }

    pub fn scene_id(&self) -> Option<i64> {
        self.scene.id
    }

    pub fn handle_perms(&mut self, user: i64, event: PermsEvent) -> bool {
        self.perms.handle_event(user, event)
    }

    pub fn owner_is(&self, user: i64) -> bool {
        matches!(self.perms.get_role(user), perms::Role::Owner)
    }

    /// Given a user ID and that users name, find a layer with that users name
    /// or create one and return it. If that user is the game owner, don't do
    /// this and just return (None, None).
    fn player_layer(&mut self, user: i64) -> (Option<SceneEvent>, Option<scene::Id>) {
        let Some(name) = self.users.get(&user) else {
            warning(format!(
                "(Game: {}) Couldn't find player (Id: {}) name.",
                self.key, user
            ));
            return (None, None);
        };

        if self.perms.get_role(user) == perms::Role::Owner {
            return (None, None);
        }

        for layer in &self.scene.layers {
            if layer.title.eq(name) {
                return (None, Some(layer.id));
            }
        }

        if let Some(event) = self.scene.new_layer(name, Scene::FOREGROUND_Z) {
            let id = if let &SceneEvent::LayerNew(id, ..) = &event {
                id
            } else {
                0
            };

            (Some(event), Some(id))
        } else {
            (None, None)
        }
    }

    /// Adds a player to the permissions set up and creates a layer for that
    /// player if none exists.
    ///
    /// Returns a tuple of (perms_event, scene_event, player_layer)
    pub fn add_player(
        &mut self,
        user: i64,
        name: &str,
    ) -> (Vec<PermsEvent>, Option<SceneEvent>, Option<scene::Id>) {
        self.users.insert(user, name.to_string());

        let mut perms = Vec::new();
        if let Some(event) =
            self.perms
                .role_change(perms::CANONICAL_UPDATER, user, perms::Role::Player)
        {
            perms.push(event);
        }

        let (scene, layer) = self.player_layer(user);
        if let Some(id) = layer
            && let Some(event) = self.perms.grant_override(user, id)
        {
            perms.push(event);
        }

        (perms, scene, layer)
    }

    /// Handle removal of a layer by recreating a player layer if needed.
    pub fn handle_remove_layer(
        &mut self,
        event: SceneEvent,
    ) -> Option<(i64, Id, Option<SceneEvent>)> {
        let SceneEvent::LayerRemove(layer) = event else {
            return None;
        };

        if let Some(layer) = self.scene.removed_layers.iter().find(|l| l.id == layer) {
            if let Some(user) = self
                .users
                .iter()
                .find(|(_, name)| name == &&layer.title)
                .map(|(user, _)| *user)
            {
                if let (event, Some(id)) = self.player_layer(user) {
                    return Some((user, id, event));
                }
            }
        }

        None
    }

    pub fn handle_event(&mut self, user: i64, event: SceneEvent) -> bool {
        let layer = self.scene.event_layer(&event);
        self.perms.permitted(user, &event, layer) && self.scene.apply_event(event.clone())
    }

    pub fn replace_scene(&mut self, scene: Scene, owner: i64) {
        self.scene = scene;
        self.scene.canon();

        let mut perms = Perms::new();
        perms.set_owner(owner);
        self.perms = perms;
    }

    pub fn server_scene(&self) -> Scene {
        self.scene.clone()
    }

    pub fn client_scene(&mut self) -> Scene {
        self.scene.non_canon()
    }

    pub fn client_perms(&mut self) -> Perms {
        self.perms.clone()
    }
}

#[cfg(test)]
mod test {
    use scene::{comms::SceneEvent, Rect, Scene, Sprite};

    use super::Game;

    #[test]
    fn test_permissions() {
        let project = 1;
        let owner = 2;
        let owner_layer = 3;
        let player = 4;
        let mut game = Game::new(project, Scene::new(), owner, "abcdefgh");

        // Owner should be able to add a new layer and a sprite to that layer.
        let owner_sprite = 6;
        assert!(game.handle_event(owner, SceneEvent::LayerNew(owner_layer, "extra".into(), 1)));
        assert!(game.handle_event(
            owner,
            SceneEvent::SpriteNew(Sprite::new(owner_sprite, None), owner_layer)
        ));

        // Adding a new player should create a layer for that player.
        let (perms, layer_event, layer_opt) = game.add_player(player, "player");
        assert!(!perms.is_empty());
        assert!(layer_event.is_some());
        assert!(layer_opt.is_some());
        let player_layer = layer_opt.unwrap();

        // Player should be able to add a sprite to their own player, but not
        // the owner's layer.
        let player_sprite = 5;
        assert!(!game.handle_event(
            player,
            SceneEvent::SpriteNew(Sprite::new(player_sprite, None), owner_layer)
        ));
        assert!(game.handle_event(
            player,
            SceneEvent::SpriteNew(Sprite::new(player_sprite, None), player_layer)
        ));

        // Player should be able to modify their sprite, but not the owner's.
        let from = game.scene.sprite(player_sprite).unwrap().rect;
        assert!(game.handle_event(
            player,
            SceneEvent::SpriteMove(player_sprite, from, Rect::new(1., 1., 1., 1.))
        ));
        let from = game.scene.sprite(owner_sprite).unwrap().rect;
        assert!(!game.handle_event(
            player,
            SceneEvent::SpriteMove(owner_sprite, from, Rect::new(1., 1., 1., 1.))
        ));

        // Player should be able to remove their sprite, but not the owner's.
        assert!(game.handle_event(
            player,
            SceneEvent::SpriteRemove(player_sprite, player_layer)
        ));
        assert!(!game.handle_event(player, SceneEvent::SpriteRemove(owner_sprite, owner_layer)));

        // Owner should be able to remove their sprite.
        assert!(game.handle_event(owner, SceneEvent::SpriteRemove(owner_sprite, owner_layer)));

        // All sprites should be removed.
        assert!(game.scene.sprite(player_sprite).is_none());
        assert!(game.scene.sprite(owner_sprite).is_none());
    }
}
