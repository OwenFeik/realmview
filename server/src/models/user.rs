use sqlx::{FromRow, SqlitePool};

#[derive(Debug, FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub salt: String,
    pub hashed_password: String,
    pub recovery_key: String,
    pub created_time: i64,
}

impl User {
    pub async fn register(
        pool: &SqlitePool,
        username: &str,
        salt: &str,
        hashed_password: &str,
        recovery_key: &str,
        created_time: u64,
    ) -> anyhow::Result<i64> {
        let id = sqlx::query(
            "INSERT INTO users (username, salt, hashed_password, recovery_key, created_time) VALUES (?1, ?2, ?3, ?4, ?5);"
        )
            .bind(username)
            .bind(salt)
            .bind(hashed_password)
            .bind(recovery_key)
            .bind(created_time as i64)
            .execute(pool)
            .await?
            .last_insert_rowid();

        Ok(id)
    }

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

    /// Given a valid session key, return the associated user. None if session
    /// has expired.
    pub async fn get_by_session(
        pool: &SqlitePool,
        session_key: &str,
    ) -> anyhow::Result<Option<User>> {
        let user = sqlx::query_as(concat!(
            "SELECT u.id, u.username, u.salt, u.hashed_password, ",
            "u.recovery_key, u.created_time FROM users u LEFT JOIN ",
            "user_sessions us ON us.user = u.id WHERE us.session_key = ?1",
            " AND us.end_time IS NULL;"
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

    pub async fn username_taken(pool: &SqlitePool, username: &str) -> anyhow::Result<bool> {
        let row = sqlx::query("SELECT id FROM users WHERE username = ?1;")
            .bind(username)
            .fetch_optional(pool)
            .await?;

        Ok(row.is_some())
    }
}
