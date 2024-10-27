use crate::{Id, Scene};

pub struct Project {
    pub id: Id,
    pub key: String,
    pub title: String,
    pub scenes: Vec<Scene>,
}
