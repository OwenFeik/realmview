use sqlx::SqlitePool;
use uuid::Uuid;

use super::{format_uuid, generate_uuid, User, UserSession};
use crate::utils::timestamp_s;

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

    pub async fn get_by_id(pool: &SqlitePool, uuid: Uuid) -> anyhow::Result<Option<User>> {
        let user = sqlx::query_as("SELECT * FROM users WHERE id = ?1;")
            .bind(format_uuid(uuid))
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

impl UserSession {
    pub async fn get(pool: &SqlitePool, session: Uuid) -> anyhow::Result<Option<UserSession>> {
        let user_session = sqlx::query_as("SELECT * FROM user_sessions WHERE uuid = ?1;")
            .bind(format_uuid(session))
            .fetch_optional(pool)
            .await?;
        Ok(user_session)
    }

    pub async fn create(pool: &SqlitePool, user: &User) -> anyhow::Result<Uuid> {
        let session: Self = sqlx::query_as(
            "INSERT INTO user_sessions (uuid, user, start_time) VALUES (?1, ?2, ?3) RETURNING *;",
        )
        .bind(format_uuid(generate_uuid()))
        .bind(format_uuid(user.uuid))
        .bind(timestamp_s()? as i64)
        .fetch_one(pool)
        .await?;
        Ok(session.uuid)
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
        User::get_by_id(pool, self.uuid).await
    }
}
