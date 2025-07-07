use serde_derive::{Deserialize, Serialize};

use crate::Project;

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
pub struct ProjectSaveResponse {
    pub message: String,
    pub success: bool,
    pub project_old: ProjectListEntry,
    pub project_new: ProjectListEntry,
}

#[derive(Serialize, Deserialize)]
pub struct ProjectListResponse {
    pub message: String,
    pub success: bool,
    pub list: Vec<ProjectListEntry>,
}

pub fn update_project_from_save(proj: &mut Project, old: ProjectListEntry, new: ProjectListEntry) {
    if let Ok(uuid) = uuid::Uuid::parse_str(&old.uuid)
        && uuid == proj.uuid
        && let Ok(uuid) = uuid::Uuid::parse_str(&new.uuid)
    {
        proj.uuid = uuid;
        proj.scenes
            .iter_mut()
            .zip(old.scene_list.iter().zip(new.scene_list.iter()))
            .for_each(|(proj_scene, (old, new))| {
                if let Ok(uuid) = uuid::Uuid::parse_str(&old.uuid)
                    && uuid == proj_scene.uuid
                    && let Ok(uuid) = uuid::Uuid::parse_str(&new.uuid)
                {
                    proj_scene.uuid = uuid;
                }
            });
    }
}
