use scene::{Scene, comms::SceneEvent};

use super::perms::Perms;

pub struct Game {
    scene: Scene,
    perms: Perms,
}

impl Game {
    pub fn new(mut scene: Scene) -> Self {
        scene.canon();
        Self { scene, perms: Perms::new() }
    }

    pub fn handle_event(&mut self, user: i64, event: SceneEvent) -> bool {
        if self.perms.permitted(user, &event) {
            self.scene.apply_event(event)
        }
        else {
            false
        }
    }
}
