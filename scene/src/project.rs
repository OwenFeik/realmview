use uuid::Uuid;

use crate::Scene;

#[derive(Clone)]
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

    pub fn with_uuid(self, uuid: Uuid) -> Self {
        Self {
            uuid: uuid,
            title: self.title,
            scenes: self.scenes,
        }
    }

    pub fn titled(uuid: Uuid, title: String) -> Self {
        Self {
            uuid,
            title,
            scenes: Vec::new(),
        }
    }

    pub fn new_scene(&mut self) -> &Scene {
        self.scenes.push(Scene::new(self.uuid));
        self.scenes.last().unwrap()
    }

    pub fn get_scene(&self, scene: Uuid) -> Option<&Scene> {
        self.scenes.iter().find(|s| s.uuid == scene)
    }

    pub fn delete_scene(&mut self, scene: Uuid) -> Result<(), String> {
        let n = self.scenes.len();
        self.scenes.retain(|sc| sc.uuid != scene);
        if self.scenes.len() >= n {
            Err("Scene not found".into())
        } else {
            Ok(())
        }
    }

    pub fn update_scene(&mut self, scene: Scene) -> Result<(), String> {
        if let Some(to_update) = self.scenes.iter_mut().find(|s| s.uuid == scene.uuid) {
            *to_update = scene;
            Ok(())
        } else {
            Err("Scene not found".into())
        }
    }

    pub fn default_scene(&mut self) -> &Scene {
        if self.scenes.is_empty() {
            self.scenes.push(Scene::new(self.uuid))
        }
        self.scenes.first().unwrap()
    }
}

impl std::fmt::Debug for Project {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Project")
            .field("uuid", &self.uuid)
            .field("title", &self.title)
            .field("scenes", &self.scenes.len())
            .finish()
    }
}
