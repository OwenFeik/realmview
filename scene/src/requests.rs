use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SceneListEntry {
    pub uuid: String,
    pub title: String,
    pub updated_time: u64,
    pub thumbnail: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ProjectListEntry {
    pub uuid: String,
    pub title: String,
    pub updated_time: u64,
    pub scene_list: Vec<SceneListEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct ProjectResponse {
    pub message: String,
    pub success: bool,
    pub project: ProjectListEntry,
}

#[derive(Serialize, Deserialize)]
pub struct ProjectListResponse {
    pub message: String,
    pub success: bool,
    pub list: Vec<ProjectListEntry>,
}
