use crate::scene::{
    comms::{PermsEvent, SceneEvent},
    perms::{self, Perms},
    Scene,
};

pub struct Game {
    pub key: String,
    pub project: i64,

    scene: Scene,
    perms: Perms,
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

    /// Given a user ID and that users name, find a layer with that users name
    /// or create one and return it. If that user is the game owner, don't do
    /// this and just return (None, None).
    fn player_layer(&mut self, user: i64, name: &str) -> (Option<SceneEvent>, Option<scene::Id>) {
        if self.perms.get_role(user) == perms::Role::Owner {
            return (None, None);
        }

        for layer in &self.scene.layers {
            if layer.title == name {
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
        let perms = self
            .perms
            .role_change(perms::CANONICAL_UPDATER, user, perms::Role::Player);
        let (scene, layer) = self.player_layer(user, name);
        (perms, scene, layer)
    }

    pub fn handle_event(
        &mut self,
        user: i64,
        event: SceneEvent,
    ) -> (bool, Option<Vec<PermsEvent>>) {
        if self
            .perms
            .permitted(user, &event, self.scene.event_layer(&event))
        {
            let overrides = self.perms.ownership_overrides(user, &event);
            if self.scene.apply_event(event) {
                return (true, overrides);
            }
        }
        (false, None)
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
