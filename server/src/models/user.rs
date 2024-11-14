use sqlx::prelude::FromRow;
use uuid::Uuid;

use super::{timestamp_s, timestamp_to_system, Conn};
use crate::{
    crypto::{from_hex_string, to_hex_string, Key},
    utils::{err, format_uuid, generate_uuid, parse_uuid, Res},
};

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

    pub async fn username_taken(conn: &mut Conn, username: &str) -> Res<bool> {
        sqlx::query!("SELECT uuid FROM users WHERE username = ?1;", username)
            .fetch_optional(conn)
            .await
            .map_err(|e| e.to_string())
            .map(|opt| opt.is_some())
    }

    #[cfg(test)]
    pub async fn generate(conn: &mut Conn) -> Res<Self> {
        UserAuth::generate(conn).await.map(Self::from)
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
            &to_hex_string(salt)?,
            &to_hex_string(hashed_password)?,
            &to_hex_string(recovery_key)?,
        )
        .await
        .and_then(Self::try_from)
    }

    #[cfg(test)]
    pub async fn generate(conn: &mut Conn) -> Res<Self> {
        let salt = crate::crypto::generate_salt()?;
        let hashed_password = crate::crypto::hash_password(&salt, "password");
        let recovery_key = crate::crypto::generate_salt()?;
        Self::register(conn, "test", &salt, &hashed_password, &recovery_key)
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
            created_time: timestamp_to_system(value.created_time),
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
    pub uuid: Uuid,
    pub user: Uuid,
    pub start: std::time::SystemTime,
    pub end: Option<std::time::SystemTime>,
}

impl UserSession {
    pub async fn create(conn: &mut Conn, user: Uuid) -> Res<Self> {
        create_user_session(conn, user)
            .await
            .and_then(Self::try_from)
    }

    pub async fn get_with_user(conn: &mut Conn, session: Uuid) -> Res<Option<(Self, User)>> {
        if let Some((user_row, session_row)) = get_user_with_session(conn, session).await? {
            let user = User::try_from(user_row)?;
            let session = UserSession::try_from(session_row)?;
            Ok(Some((session, user)))
        } else {
            Ok(None)
        }
    }

    pub async fn end(self, conn: &mut Conn) -> Res<()> {
        let end_time = timestamp_s();
        let uuid = format_uuid(self.uuid);
        sqlx::query!(
            "UPDATE user_sessions SET end_time = ?1 WHERE uuid = ?2",
            end_time,
            uuid
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
            uuid: parse_uuid(&value.uuid)?,
            user: parse_uuid(&value.user)?,
            start: timestamp_to_system(value.start_time),
            end: value.end_time.map(timestamp_to_system),
        })
    }
}

#[derive(sqlx::FromRow)]
struct UserSessionRow {
    uuid: String,
    user: String,
    start_time: i64,
    end_time: Option<i64>,
}

async fn lookup_user_session(conn: &mut Conn, session: Uuid) -> Res<Option<UserSessionRow>> {
    let session = format_uuid(session);
    sqlx::query_as!(
        UserSessionRow,
        "SELECT * FROM user_sessions WHERE uuid = ?1;",
        session
    )
    .fetch_optional(conn)
    .await
    .map_err(|e| e.to_string())
}

async fn create_user_session(conn: &mut Conn, user: Uuid) -> Res<UserSessionRow> {
    let uuid = format_uuid(generate_uuid());
    let user = format_uuid(user);
    let start_time = timestamp_s();
    sqlx::query_as!(
        UserSessionRow,
        "INSERT INTO user_sessions (uuid, user, start_time) VALUES (?1, ?2, ?3) RETURNING *;",
        uuid,
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
    session: Uuid,
) -> Res<Option<(UserRow, UserSessionRow)>> {
    #[derive(FromRow)]
    struct QueryRow {
        user_uuid: String,
        username: String,
        salt: String,
        hashed_password: String,
        recovery_key: String,
        created_time: i64,
        session_uuid: String,
        start_time: i64,
        end_time: Option<i64>,
    }

    let session_uuid = format_uuid(session);
    let row = sqlx::query_as!(
        QueryRow,
        "
        SELECT
            users.uuid as user_uuid,
            username,
            salt,
            hashed_password,
            recovery_key,
            created_time,
            user_sessions.uuid as session_uuid,
            start_time,
            end_time
        FROM users LEFT JOIN user_sessions ON user_sessions.user = users.uuid
        WHERE user_sessions.uuid = ?1 AND user_sessions.end_time IS NULL;
        ",
        session_uuid
    )
    .fetch_optional(conn)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.map(|row| {
        (
            UserRow {
                uuid: row.user_uuid.clone(),
                username: row.username,
                salt: row.salt,
                hashed_password: row.hashed_password,
                recovery_key: row.recovery_key,
                created_time: row.created_time,
            },
            UserSessionRow {
                uuid: row.session_uuid,
                user: row.user_uuid,
                start_time: row.start_time,
                end_time: row.end_time,
            },
        )
    }))
}
