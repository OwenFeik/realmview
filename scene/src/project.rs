use uuid::Uuid;

use crate::Scene;

pub struct Project {
    pub uuid: Uuid,
    pub title: String,
    pub scenes: Vec<Scene>,
}

impl Project {
    pub fn new(uuid: Uuid) -> Self {
        Self {
            uuid,
            title: "Untitled".to_string(),
            scenes: Vec::new(),
        }
    }

    pub fn get_scene(&self, scene: Uuid) -> Option<&Scene> {
        self.scenes.iter().find(|s| s.uuid == Some(scene))
    }

    pub fn default_scene(&mut self) -> &Scene {
        if self.scenes.is_empty() {
            self.scenes.push(Scene::new(self.uuid))
        }
        self.scenes.first().unwrap()
    }
}
