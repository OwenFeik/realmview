use sqlx::{SqliteConnection, SqlitePool};
use uuid::Uuid;

use super::{timestamp_s, User, UserSession};
use crate::utils::{format_uuid, generate_uuid, Res};

impl User {
    pub async fn register(
        pool: &SqlitePool,
        username: &str,
        salt: &str,
        hashed_password: &str,
        recovery_key: &str,
        created_time: u64,
    ) -> Res<Uuid> {
        let uuid = generate_uuid();
        sqlx::query(
            "INSERT INTO users (uuid, username, salt, hashed_password, recovery_key, created_time) VALUES (?1, ?2, ?3, ?4, ?5);"
        )
            .bind(format_uuid(uuid))
            .bind(username)
            .bind(salt)
            .bind(hashed_password)
            .bind(recovery_key)
            .bind(created_time as i64)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;

        Ok(uuid)
    }

    pub async fn get(pool: &SqlitePool, username: &str) -> Res<Option<User>> {
        let user = sqlx::query_as("SELECT * FROM users WHERE username = ?1;")
            .bind(username)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(user)
    }

    pub async fn lookup(pool: &SqlitePool, uuid: Uuid) -> Res<Option<User>> {
        let user = sqlx::query_as("SELECT * FROM users WHERE id = ?1;")
            .bind(format_uuid(uuid))
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(user)
    }

    pub async fn get_by_uuid(conn: &mut SqliteConnection, uuid: Uuid) -> Res<Self> {
        sqlx::query_as(
            "
            SELECT (uuid, username, salt, hashed_password, recovery_key, created_time)
            FROM users WHERE uuid = ?1;
            ",
        )
        .bind(format_uuid(uuid))
        .fetch_one(conn)
        .await
        .map_err(|e| e.to_string())
    }

    /// Given a valid session key, return the associated user. None if session
    /// has expired.
    pub async fn get_by_session(pool: &SqlitePool, session_key: &str) -> Res<Option<User>> {
        let user = sqlx::query_as(concat!(
            "SELECT u.uuid, u.username, u.salt, u.hashed_password, ",
            "u.recovery_key, u.created_time FROM users u LEFT JOIN ",
            "user_sessions us ON us.user = u.uuid WHERE us.session_key = ?1",
            " AND us.end_time IS NULL;"
        ))
        .bind(session_key)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(user)
    }

    pub fn relative_save_path(&self) -> String {
        format!("/saves/{}", &self.username)
    }

    pub fn relative_upload_path(&self) -> String {
        format!("/uploads/{}", &self.username)
    }

    pub fn absolute_upload_path(&self, content_dir: &str) -> String {
        format!("{}/{}", content_dir, &self.relative_upload_path())
    }

    pub async fn username_taken(pool: &SqlitePool, username: &str) -> Res<bool> {
        let row = sqlx::query("SELECT id FROM users WHERE username = ?1;")
            .bind(username)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

        Ok(row.is_some())
    }
}

impl UserSession {
    pub async fn get(pool: &SqlitePool, session: Uuid) -> Res<Option<UserSession>> {
        let user_session = sqlx::query_as("SELECT * FROM user_sessions WHERE uuid = ?1;")
            .bind(format_uuid(session))
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(user_session)
    }

    pub async fn create(pool: &SqlitePool, user: &User) -> Res<Uuid> {
        let session: Self = sqlx::query_as(
            "INSERT INTO user_sessions (uuid, user, start_time) VALUES (?1, ?2, ?3) RETURNING *;",
        )
        .bind(format_uuid(generate_uuid()))
        .bind(format_uuid(user.uuid))
        .bind(timestamp_s())
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(session.uuid)
    }

    pub async fn end(pool: &SqlitePool, session_key: &str) -> Res<bool> {
        let rows_affected =
            sqlx::query("UPDATE user_sessions SET end_time = ?1 WHERE session_key = ?2")
                .bind(timestamp_s())
                .bind(session_key)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?
                .rows_affected();

        Ok(rows_affected > 0)
    }

    pub async fn user(&self, pool: &SqlitePool) -> Res<Option<User>> {
        User::lookup(pool, self.uuid).await
    }
}
