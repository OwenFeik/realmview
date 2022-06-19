use scene::comms::SceneEvent;

pub struct Perms {}

impl Perms {
    pub fn new() -> Self {
        Perms {}
    }

    pub fn permitted(&self, user: i64, event: &SceneEvent) -> bool {
        true
    }
}
