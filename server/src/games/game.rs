use std::collections::HashMap;

use scene::{comms::ServerEvent, Id};

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
    ) -> (Option<PermsEvent>, Option<SceneEvent>, Option<scene::Id>) {
        self.users.insert(user, name.to_string());
        let perms = self
            .perms
            .role_change(perms::CANONICAL_UPDATER, user, perms::Role::Player);
        let (scene, layer) = self.player_layer(user);
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

    pub fn handle_event(&mut self, user: i64, event: SceneEvent) -> (bool, Option<ServerEvent>) {
        if self.perms.permitted(user, &event) && self.scene.apply_event(event.clone()) {
            let perms_event = self
                .perms
                .created(user, &event)
                .map(ServerEvent::PermsUpdate);

            (true, perms_event)
        } else {
            (false, None)
        }
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
