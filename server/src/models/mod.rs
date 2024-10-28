use sqlx::{FromRow, SqlitePool};

#[cfg(test)]
mod tests;

mod media;
mod project;
mod user;

pub use media::Media;
pub use project::ProjectRecord as Project;
pub use project::SceneRecord as Scene;
pub use user::User;

use crate::crypto::generate_salt;
use crate::crypto::to_hex_string;
use crate::utils::timestamp_s;

#[derive(FromRow)]
pub struct UserSession {
    pub id: i64,

    #[sqlx(rename = "user")]
    pub user_id: i64,

    pub session_key: String,
    pub active: bool,
    pub start_time: i64,
    pub end_time: Option<i64>,
}

impl UserSession {
    pub async fn get(pool: &SqlitePool, session_key: &str) -> anyhow::Result<Option<UserSession>> {
        let user_session = sqlx::query_as("SELECT * FROM user_sessions WHERE session_key = ?1;")
            .bind(session_key)
            .fetch_optional(pool)
            .await?;
        Ok(user_session)
    }

    pub async fn create(pool: &SqlitePool, user: &User) -> anyhow::Result<String> {
        let session_key = to_hex_string(&generate_salt()?)?;

        sqlx::query(
            "INSERT INTO user_sessions (user, session_key, start_time) VALUES (?1, ?2, ?3);",
        )
        .bind(user.id)
        .bind(session_key.as_str())
        .bind(timestamp_s()? as i64)
        .execute(pool)
        .await?;

        Ok(session_key)
    }

    pub async fn end(pool: &SqlitePool, session_key: &str) -> anyhow::Result<bool> {
        let rows_affected =
            sqlx::query("UPDATE user_sessions SET end_time = ?1 WHERE session_key = ?2")
                .bind(timestamp_s()? as i64)
                .bind(session_key)
                .execute(pool)
                .await?
                .rows_affected();

        Ok(rows_affected > 0)
    }

    pub async fn user(&self, pool: &SqlitePool) -> anyhow::Result<Option<User>> {
        User::get_by_id(pool, self.user_id).await
    }
}
