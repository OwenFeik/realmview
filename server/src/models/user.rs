use sqlx::prelude::FromRow;
use uuid::Uuid;

use super::{timestamp_s, timestamp_to_system, Conn};
use crate::{
    crypto::{from_hex_string, generate_key, to_hex_string, Key},
    utils::{err, format_uuid, generate_uuid, parse_uuid, Res},
};

#[derive(Debug)]
pub struct User {
    pub uuid: Uuid,
    pub username: String,
}

impl User {
    pub async fn get_by_uuid(conn: &mut Conn, uuid: Uuid) -> Res<Self> {
        match lookup(conn, uuid).await? {
            Some(record) => Self::try_from(record),
            None => err("User does not exist."),
        }
    }

    pub async fn username_taken(conn: &mut Conn, username: &str) -> Res<bool> {
        sqlx::query!("SELECT uuid FROM users WHERE username = ?1;", username)
            .fetch_optional(conn)
            .await
            .map_err(|e| e.to_string())
            .map(|opt| opt.is_some())
    }

    #[cfg(test)]
    pub async fn generate(conn: &mut Conn) -> Self {
        UserAuth::generate(conn)
            .await
            .map(Self::from)
            .expect("Failed to generate user.")
    }

    #[cfg(test)]
    pub async fn session(&self, conn: &mut Conn) -> actix_web::cookie::Cookie {
        let session = UserSession::create(conn, self.uuid)
            .await
            .expect("Failed to create user session.");
        let mut cookie = actix_web::cookie::Cookie::named(crate::req::session::COOKIE_NAME);
        cookie.set_value(to_hex_string(&session.session_key));
        cookie
    }
}

impl TryFrom<UserRow> for User {
    type Error = String;

    fn try_from(value: UserRow) -> Result<Self, Self::Error> {
        Ok(Self {
            uuid: parse_uuid(&value.uuid)?,
            username: value.username,
        })
    }
}

impl From<UserAuth> for User {
    fn from(value: UserAuth) -> Self {
        Self {
            uuid: value.uuid,
            username: value.username,
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
}

impl UserAuth {
    pub async fn get_by_username(conn: &mut Conn, username: &str) -> Res<Self> {
        match lookup_by_username(conn, username).await? {
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
            &to_hex_string(salt),
            &to_hex_string(hashed_password),
            &to_hex_string(recovery_key),
        )
        .await
        .and_then(Self::try_from)
    }

    #[cfg(test)]
    pub const GENERATED_USER_PASSWORD: &'static str = "password";

    #[cfg(test)]
    pub async fn generate(conn: &mut Conn) -> Res<Self> {
        // Generate user names with an incrementing counter to ensure
        // uniqueness.
        static I: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let i = I.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let username = format!("test{i}");

        // Users all use GENERATED_USER_PASSWORD, with a randomly generated
        // salt.
        let salt = crate::crypto::generate_key()?;
        let hashed_password = crate::crypto::hash_password(&salt, Self::GENERATED_USER_PASSWORD);
        let recovery_key = crate::crypto::generate_key()?;

        Self::register(conn, &username, &salt, &hashed_password, &recovery_key)
            .await
            .map(Self::from)
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
        })
    }
}

#[derive(FromRow)]
struct UserRow {
    uuid: String,
    username: String,
    salt: String,
    hashed_password: String,
    recovery_key: String,
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
    sqlx::query_as!(
        UserRow,
        "
        INSERT INTO users (uuid, username, salt, hashed_password, recovery_key)
        VALUES (?1, ?2, ?3, ?4, ?5) RETURNING *;
        ",
        uuid,
        username,
        salt,
        hashed_password,
        recovery_key
    )
    .fetch_one(conn)
    .await
    .map_err(|e| e.to_string())
}

async fn lookup_by_username(conn: &mut Conn, username: &str) -> Res<Option<UserRow>> {
    sqlx::query_as!(
        UserRow,
        "SELECT * FROM users WHERE username = ?1;",
        username
    )
    .fetch_optional(conn)
    .await
    .map_err(|e| e.to_string())
}

#[derive(Debug)]
pub struct UserSession {
    pub session_key: Key,
    pub user: Uuid,
    pub start: std::time::SystemTime,
    pub end: Option<std::time::SystemTime>,
}

impl UserSession {
    pub fn key_text(&self) -> String {
        to_hex_string(&self.session_key)
    }

    pub async fn create(conn: &mut Conn, user: Uuid) -> Res<Self> {
        create_user_session(conn, user)
            .await
            .and_then(Self::try_from)
    }

    pub async fn get_with_user(conn: &mut Conn, session_key: &str) -> Res<Option<(Self, User)>> {
        if let Some((user_row, session_row)) = get_user_with_session(conn, session_key).await? {
            let user = User::try_from(user_row)?;
            let session = UserSession::try_from(session_row)?;
            Ok(Some((session, user)))
        } else {
            Ok(None)
        }
    }

    pub async fn end(self, conn: &mut Conn) -> Res<()> {
        let end_time = timestamp_s();
        let session_key = to_hex_string(&self.session_key);
        sqlx::query!(
            "UPDATE user_sessions SET end_time = ?1 WHERE session_key = ?2",
            end_time,
            session_key
        )
        .execute(conn)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
    }
}

impl TryFrom<UserSessionRow> for UserSession {
    type Error = String;

    fn try_from(value: UserSessionRow) -> Result<Self, Self::Error> {
        Ok(Self {
            session_key: from_hex_string(&value.session_key)?,
            user: parse_uuid(&value.user)?,
            start: timestamp_to_system(value.start_time),
            end: value.end_time.map(timestamp_to_system),
        })
    }
}

#[derive(sqlx::FromRow)]
struct UserSessionRow {
    session_key: String,
    user: String,
    start_time: i64,
    end_time: Option<i64>,
}

async fn lookup_user_session(conn: &mut Conn, session_key: &str) -> Res<Option<UserSessionRow>> {
    sqlx::query_as!(
        UserSessionRow,
        "SELECT * FROM user_sessions WHERE session_key = ?1;",
        session_key
    )
    .fetch_optional(conn)
    .await
    .map_err(|e| e.to_string())
}

async fn create_user_session(conn: &mut Conn, user: Uuid) -> Res<UserSessionRow> {
    let session_key = to_hex_string(&generate_key()?);
    let user = format_uuid(user);
    let start_time = timestamp_s();
    sqlx::query_as!(
        UserSessionRow,
        "
        INSERT INTO user_sessions (session_key, user, start_time)
        VALUES (?1, ?2, ?3) RETURNING *;
        ",
        session_key,
        user,
        start_time
    )
    .fetch_one(conn)
    .await
    .map_err(|e| e.to_string())
}

/// Given a valid session key, return the associated user. None if session
/// has expired.
async fn get_user_with_session(
    conn: &mut Conn,
    session: &str,
) -> Res<Option<(UserRow, UserSessionRow)>> {
    #[derive(FromRow)]
    struct QueryRow {
        uuid: String,
        username: String,
        salt: String,
        hashed_password: String,
        recovery_key: String,
        session_key: String,
        start_time: i64,
        end_time: Option<i64>,
    }

    let row = sqlx::query_as!(
        QueryRow,
        "
        SELECT
            uuid,
            username,
            salt,
            hashed_password,
            recovery_key,
            session_key,
            start_time,
            end_time
        FROM users LEFT JOIN user_sessions ON user_sessions.user = users.uuid
        WHERE user_sessions.session_key = ?1 AND user_sessions.end_time IS NULL;
        ",
        session
    )
    .fetch_optional(conn)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.map(|row| {
        (
            UserRow {
                uuid: row.uuid.clone(),
                username: row.username,
                salt: row.salt,
                hashed_password: row.hashed_password,
                recovery_key: row.recovery_key,
            },
            UserSessionRow {
                session_key: row.session_key,
                user: row.uuid,
                start_time: row.start_time,
                end_time: row.end_time,
            },
        )
    }))
}
