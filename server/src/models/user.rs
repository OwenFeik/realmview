use sqlx::{prelude::FromRow, SqliteConnection, SqlitePool};
use uuid::Uuid;

use super::{timestamp_s, timestamp_to_system, Conn, UserSession};
use crate::{
    crypto::{from_hex_string, to_hex_string, Key},
    utils::{err, format_uuid, generate_uuid, parse_uuid, Res},
};

type Pool = SqlitePool;

#[derive(Debug)]
pub struct User {
    pub uuid: Uuid,
    pub username: String,
    pub created_time: std::time::SystemTime,
}

impl User {
    pub async fn get_by_uuid(conn: &mut Conn, uuid: Uuid) -> Res<Self> {
        match lookup(conn, uuid).await? {
            Some(record) => Self::try_from(record),
            None => err("User does not exist."),
        }
    }

    pub async fn username_taken(pool: &SqlitePool, username: &str) -> Res<bool> {
        sqlx::query!("SELECT uuid FROM users WHERE username = ?1;", username)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())
            .map(|opt| opt.is_some())
    }

    #[cfg(test)]
    pub async fn generate(conn: &mut Conn) -> Res<Self> {
        let salt = crate::crypto::generate_salt()?;
        let hashed_password = crate::crypto::hash_password(&salt, "password");
        let recovery_key = crate::crypto::generate_salt()?;
        UserAuth::register(conn, "test", &salt, &hashed_password, &recovery_key)
            .await
            .map(Self::from)
    }
}

impl TryFrom<UserRow> for User {
    type Error = String;

    fn try_from(value: UserRow) -> Result<Self, Self::Error> {
        Ok(Self {
            uuid: parse_uuid(&value.uuid)?,
            username: value.username,
            created_time: timestamp_to_system(value.created_time),
        })
    }
}

impl From<UserAuth> for User {
    fn from(value: UserAuth) -> Self {
        Self {
            uuid: value.uuid,
            username: value.username,
            created_time: value.created_time,
        }
    }
}

#[derive(Debug)]
pub struct UserAuth {
    pub uuid: Uuid,
    pub username: String,
    pub salt: Key,
    pub hashed_password: Key,
    pub recovery_key: Key,
    pub created_time: std::time::SystemTime,
}

impl UserAuth {
    pub async fn get_by_username(pool: &Pool, username: &str) -> Res<Self> {
        match lookup_by_username(pool, username).await? {
            Some(record) => Self::try_from(record),
            None => Err(format!("User {username} does not exist.")),
        }
    }

    pub async fn register(
        conn: &mut Conn,
        username: &str,
        salt: &Key,
        hashed_password: &Key,
        recovery_key: &Key,
    ) -> Res<Self> {
        register(
            conn,
            username,
            &to_hex_string(salt)?,
            &to_hex_string(hashed_password)?,
            &to_hex_string(recovery_key)?,
        )
        .await
        .and_then(Self::try_from)
    }
}

impl TryFrom<UserRow> for UserAuth {
    type Error = String;

    fn try_from(value: UserRow) -> Result<Self, Self::Error> {
        Ok(Self {
            uuid: parse_uuid(&value.uuid)?,
            username: value.username,
            salt: from_hex_string(&value.salt)?,
            hashed_password: from_hex_string(&value.hashed_password)?,
            recovery_key: from_hex_string(&value.hashed_password)?,
            created_time: timestamp_to_system(value.created_time),
        })
    }
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

#[derive(FromRow)]
struct UserRow {
    uuid: String,
    username: String,
    salt: String,
    hashed_password: String,
    recovery_key: String,
    created_time: i64,
}

async fn lookup(conn: &mut Conn, uuid: Uuid) -> Res<Option<UserRow>> {
    let uuid = format_uuid(uuid);
    sqlx::query_as!(UserRow, "SELECT * FROM users WHERE uuid = ?1;", uuid)
        .fetch_optional(conn)
        .await
        .map_err(|e| e.to_string())
}

async fn register(
    conn: &mut Conn,
    username: &str,
    salt: &str,
    hashed_password: &str,
    recovery_key: &str,
) -> Res<UserRow> {
    let uuid = format_uuid(generate_uuid());
    let created_time = timestamp_s();
    sqlx::query_as!(
        UserRow,
        "
        INSERT INTO users (uuid, username, salt, hashed_password, recovery_key, created_time)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6) RETURNING *;
        ",
        uuid,
        username,
        salt,
        hashed_password,
        recovery_key,
        created_time
    )
    .fetch_one(conn)
    .await
    .map_err(|e| e.to_string())
}

async fn lookup_by_username(pool: &Pool, username: &str) -> Res<Option<UserRow>> {
    sqlx::query_as!(
        UserRow,
        "SELECT * FROM users WHERE username = ?1;",
        username
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())
}
