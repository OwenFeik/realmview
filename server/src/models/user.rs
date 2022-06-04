use sqlx::{FromRow, SqlitePool};

#[derive(FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub salt: String,
    pub hashed_password: String,
    pub recovery_key: String,
    pub created_time: i64,
}

impl User {
    pub async fn get(pool: &SqlitePool, username: &str) -> anyhow::Result<Option<User>> {
        let user = sqlx::query_as("SELECT * FROM users WHERE username = ?1;")
            .bind(username)
            .fetch_optional(pool)
            .await?;
        Ok(user)
    }

    pub async fn get_by_id(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<User>> {
        let user = sqlx::query_as("SELECT * FROM users WHERE id = ?1;")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(user)
    }

    pub async fn get_by_session(
        pool: &SqlitePool,
        session_key: &str,
    ) -> anyhow::Result<Option<User>> {
        let user = sqlx::query_as(concat!(
            "SELECT u.id, u.username, u.salt, u.hashed_password, ",
            "u.recovery_key, u.created_time FROM users u LEFT JOIN ",
            "user_sessions us ON us.user = u.id WHERE us.session_key = ?1;"
        ))
        .bind(session_key)
        .fetch_optional(pool)
        .await?;
        Ok(user)
    }

    pub fn relative_dir(&self) -> String {
        format!("/uploads/{}", &self.username)
    }

    pub fn upload_dir(&self, content_dir: &str) -> String {
        format!("{}/{}", content_dir, &self.relative_dir())
    }
}
