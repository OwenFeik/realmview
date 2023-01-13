use crate::{comms::SceneEvent, Id};

#[derive(Clone, serde_derive::Deserialize, serde_derive::Serialize)]
pub struct Group {
    pub id: Id,
    sprites: Vec<Id>,
}

impl Group {
    pub fn new(id: Id, sprites: Vec<Id>) -> Self {
        Group { id, sprites }
    }

    pub fn includes(&self, sprite: Id) -> bool {
        self.sprites.contains(&sprite)
    }

    pub fn add(&mut self, sprite: Id) -> SceneEvent {
        if !self.includes(sprite) {
            self.sprites.push(sprite);
        }
        SceneEvent::GroupAdd(self.id, sprite)
    }

    pub fn remove(&mut self, sprite: Id) -> SceneEvent {
        self.sprites.retain(|s| *s != sprite);
        SceneEvent::GroupRemove(self.id, sprite)
    }

    pub fn sprites(&self) -> &[Id] {
        &self.sprites
    }
}
