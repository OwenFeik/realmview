use uuid::Uuid;

use super::{Conn, User};
use crate::utils::{err, format_uuid, generate_uuid, parse_uuid, Res};

pub struct Media {
    pub uuid: Uuid,
    pub user: Uuid,
    pub relative_path: String,
    pub title: String,
    pub hashed_value: String,
    pub file_size: usize,
    pub w: f32,
    pub h: f32,
}

impl Media {
    const KEY_LENGTH: usize = 16;
    const DEFAULT_SIZE: f32 = 1.0;

    pub fn prepare<S1: ToString, S2: ToString>(
        user: &User,
        ext: &str,
        title: S1,
        hash: S2,
        size: usize,
    ) -> Self {
        let uuid = generate_uuid();
        Self {
            uuid: generate_uuid(),
            user: user.uuid,
            relative_path: format!("/uploads/{}/{}.{ext}", &user.username, format_uuid(uuid)),
            title: title.to_string(),
            hashed_value: hash.to_string(),
            file_size: size,
            w: Self::DEFAULT_SIZE,
            h: Self::DEFAULT_SIZE,
        }
    }

    pub async fn create(self, conn: &mut Conn) -> Res<Self> {
        create_media(conn, &self).await.and_then(Self::try_from)
    }

    pub async fn load(conn: &mut Conn, uuid: Uuid) -> Res<Self> {
        match lookup(conn, uuid).await? {
            Some(record) => Self::try_from(record),
            None => err("Media item does not exist."),
        }
    }

    pub async fn delete(conn: &mut Conn, uuid: Uuid) -> Res<()> {
        let uuid = format_uuid(uuid);
        sqlx::query!("DELETE FROM media WHERE uuid = ?1;", uuid)
            .execute(conn)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub async fn update(
        conn: &mut Conn,
        user: Uuid,
        uuid: Uuid,
        title: &str,
        w: f32,
        h: f32,
    ) -> Res<Self> {
        update_media(conn, user, uuid, title, w, h)
            .await
            .and_then(Self::try_from)
    }

    pub async fn user_total_size(conn: &mut Conn, user: Uuid) -> Res<usize> {
        #[derive(sqlx::FromRow)]
        struct QueryRow {
            total_file_size: Option<i64>,
        }

        let user = format_uuid(user);
        let record = sqlx::query_as!(
            QueryRow,
            "SELECT SUM(file_size) as total_file_size FROM media WHERE user = ?1;",
            user
        )
        .fetch_one(conn)
        .await
        .map_err(|e| e.to_string())?;

        Ok(record.total_file_size.unwrap_or(0) as usize)
    }

    pub async fn user_media(conn: &mut Conn, user: Uuid) -> Res<Vec<Media>> {
        user_media(conn, user)
            .await?
            .into_iter()
            .map(Self::try_from)
            .collect()
    }

    pub async fn exists(conn: &mut Conn, user: Uuid, hash: &str) -> Res<Option<String>> {
        let row_opt = sqlx::query("SELECT title FROM media WHERE user = ?1 AND hashed_value = ?2;")
            .bind(format_uuid(user))
            .bind(hash)
            .fetch_optional(conn)
            .await
            .map_err(|e| e.to_string())?;

        if let Some(row) = row_opt {
            match sqlx::Row::try_get(&row, 0) {
                Ok(s) => Ok(Some(s)),
                Err(_) => err("Database error."),
            }
        } else {
            Ok(None)
        }
    }
}

impl TryFrom<MediaRow> for Media {
    type Error = String;

    fn try_from(value: MediaRow) -> Result<Self, Self::Error> {
        Ok(Self {
            uuid: parse_uuid(&value.uuid)?,
            user: parse_uuid(&value.user)?,
            relative_path: value.relative_path,
            title: value.title,
            hashed_value: value.hashed_value,
            file_size: value.file_size as usize,
            w: value.w as f32,
            h: value.h as f32,
        })
    }
}

#[derive(sqlx::FromRow)]
struct MediaRow {
    uuid: String,
    user: String,
    relative_path: String,
    title: String,
    hashed_value: String,
    file_size: i64,
    w: f64,
    h: f64,
}

async fn lookup(conn: &mut Conn, uuid: Uuid) -> Res<Option<MediaRow>> {
    let uuid = format_uuid(uuid);
    sqlx::query_as!(MediaRow, "SELECT * FROM media WHERE uuid = ?1;", uuid)
        .fetch_optional(conn)
        .await
        .map_err(|e| format!("Media item not found: {e}"))
}

async fn create_media(conn: &mut Conn, record: &Media) -> Res<MediaRow> {
    let uuid = format_uuid(record.uuid);
    let user = format_uuid(record.user);
    let relative_path = &record.relative_path;
    let title = &record.title;
    let hashed_value = &record.hashed_value;
    let file_size = record.file_size as i64;
    let w = record.w as f64;
    let h = record.h as f64;
    sqlx::query_as!(
        MediaRow,
        "
        INSERT INTO media (
            uuid, user, relative_path, title, hashed_value, file_size, w, h
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) RETURNING *;
        ",
        uuid,
        user,
        relative_path,
        title,
        hashed_value,
        file_size,
        w,
        h
    )
    .fetch_one(conn)
    .await
    .map_err(|e| e.to_string())
}

async fn user_media(pool: &mut Conn, user: Uuid) -> Res<Vec<MediaRow>> {
    let user = format_uuid(user);
    sqlx::query_as!(MediaRow, "SELECT * FROM media WHERE user = ?1;", user)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())
}

async fn update_media(
    conn: &mut Conn,
    user: Uuid,
    uuid: Uuid,
    title: &str,
    w: f32,
    h: f32,
) -> Res<MediaRow> {
    let w = w as f64;
    let h = h as f64;
    let uuid = format_uuid(uuid);
    let user = format_uuid(user);
    sqlx::query_as!(
        MediaRow,
        "UPDATE media SET title = ?1, w = ?2, h = ?3 WHERE uuid = ?4 AND user = ?5 RETURNING *;",
        title,
        w,
        h,
        uuid,
        user
    )
    .fetch_one(conn)
    .await
    .map_err(|e| e.to_string())
}
