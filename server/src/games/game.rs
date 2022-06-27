use scene::{comms::SceneEvent, perms::Perms, Scene};

pub struct Game {
    scene: Scene,
    perms: Perms,
}

impl Game {
    pub fn new(mut scene: Scene, owner: i64) -> Self {
        scene.canon();
        let mut perms = Perms::new();
        perms.set_owner(owner);
        Self { scene, perms }
    }

    pub fn handle_event(&mut self, user: i64, event: SceneEvent) -> bool {
        if self
            .perms
            .permitted(user, &event, self.scene.event_layer(&event))
        {
            self.scene.apply_event(event)
        } else {
            false
        }
    }

    pub fn client_scene(&mut self) -> Scene {
        self.scene.non_canon()
    }
}
