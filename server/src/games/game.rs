use crate::scene::{
    comms::{PermsEvent, SceneEvent},
    perms::{self, Perms},
    Scene,
};

pub struct Game {
    pub key: String,

    scene: Scene,
    perms: Perms,
}

impl Game {
    pub fn new(mut scene: Scene, owner: i64, key: &str) -> Self {
        scene.canon();
        let mut perms = Perms::new();
        perms.set_owner(owner);
        Self {
            key: key.to_owned(),
            scene,
            perms,
        }
    }

    pub fn handle_perms(&mut self, user: i64, event: PermsEvent) -> bool {
        self.perms.handle_event(user, event)
    }

    pub fn add_player(&mut self, user: i64) -> Option<PermsEvent> {
        self.perms
            .role_change(perms::CANONICAL_UPDATER, user, perms::Role::Player)
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

    pub fn client_scene(&mut self) -> Scene {
        self.scene.non_canon()
    }

    pub fn client_perms(&mut self) -> Perms {
        self.perms.clone()
    }
}
