use uuid::Uuid;

use crate::Scene;

pub struct Project {
    pub uuid: Uuid,
    pub title: String,
    pub scenes: Vec<Scene>,
}
