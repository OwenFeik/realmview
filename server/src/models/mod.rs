use sqlx::{FromRow, SqlitePool};

mod project;
mod user;

pub use project::Project;
pub use project::SceneRecord;
pub use user::User;

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
        let user_sesion = sqlx::query_as("SELECT * FROM user_sessions WHERE session_key = ?1;")
            .bind(session_key)
            .fetch_optional(pool)
            .await?;
        Ok(user_sesion)
    }

    pub async fn user(&self, pool: &SqlitePool) -> anyhow::Result<Option<User>> {
        User::get_by_id(pool, self.user_id).await
    }
}
