#[cfg(test)]
mod tests;

mod media;
mod project;
mod scene;
mod user;

type Conn = sqlx::SqliteConnection;

pub use self::media::Media;
pub use self::project::Project;
pub use self::scene::Scene;
pub use self::user::{User, UserAuth, UserSession};

fn timestamp_s() -> i64 {
    crate::utils::timestamp_s().unwrap_or(0) as i64
}

fn timestamp_to_system(timestamp: i64) -> std::time::SystemTime {
    std::time::UNIX_EPOCH + std::time::Duration::from_secs(timestamp as u64)
}
