use uuid::Uuid;

use crate::Scene;

pub struct Project {
    pub uuid: Uuid,
    pub title: String,
    pub scenes: Vec<Scene>,
}

impl Project {
    pub fn get_scene(&self, scene: Uuid) -> Option<&Scene> {
        self.scenes.iter().find(|s| s.uuid == Some(scene))
    }
}
