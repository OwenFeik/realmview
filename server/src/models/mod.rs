#[cfg(test)]
mod tests;

mod media;
mod project;
mod user;

use sqlx::prelude::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow)]
pub struct User {
    pub uuid: Uuid,
    pub username: String,
    pub salt: String,
    pub hashed_password: String,
    pub recovery_key: String,
    pub created_time: i64,
}

#[derive(FromRow)]
pub struct UserSession {
    pub uuid: Uuid,
    pub user: Uuid,
    pub start_time: i64,
    pub end_time: Option<i64>,
}

#[derive(FromRow)]
pub struct Media {
    pub uuid: Uuid,
    pub user: Uuid,
    pub relative_path: String,
    pub title: String,
    pub hashed_value: String,
    pub file_size: i64,
    pub w: f32,
    pub h: f32,
}

#[derive(FromRow)]
pub struct Project {
    pub uuid: Uuid,
    pub user: Uuid,
    pub updated_time: i64,
    pub title: Option<String>,
}

#[derive(FromRow)]
pub struct Scene {
    pub uuid: Uuid,
    pub project: Uuid,
    pub updated_time: i64,
    pub title: Option<String>,
    pub thumbnail: Option<String>,
}

fn timestamp_s() -> i64 {
    crate::utils::timestamp_s().unwrap_or(0) as i64
}

fn format_uuid(uuid: Uuid) -> String {
    uuid.simple().to_string()
}
