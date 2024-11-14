#[cfg(test)]
mod tests;

mod media;
mod project;
mod scene;
mod user;

use sqlx::prelude::FromRow;
use sqlx::types::Uuid;

type Conn = sqlx::SqliteConnection;

pub use self::project::Project;
pub use self::scene::Scene;
pub use self::user::{User, UserAuth};

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

fn timestamp_s() -> i64 {
    crate::utils::timestamp_s().unwrap_or(0) as i64
}

fn timestamp_to_system(timestamp: i64) -> std::time::SystemTime {
    std::time::UNIX_EPOCH + std::time::Duration::from_secs(timestamp as u64)
}
