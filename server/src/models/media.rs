use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::{Conn, Media};
use crate::utils::{err, format_uuid, generate_uuid, Res};

impl Media {
    const KEY_LENGTH: usize = 16;
    const DEFAULT_SIZE: f32 = 1.0;

    pub fn prepare(
        user_uuid: Uuid,
        directory: &str,
        ext: &str,
        title: String,
        hash: String,
        size: i64,
    ) -> Self {
        let uuid = generate_uuid();
        Self {
            uuid: generate_uuid(),
            user: user_uuid,
            relative_path: format!("{directory}/{}.{ext}", format_uuid(uuid)),
            title,
            hashed_value: hash,
            file_size: size,
            w: Self::DEFAULT_SIZE,
            h: Self::DEFAULT_SIZE,
        }
    }

    pub async fn create(&self, conn: &mut Conn) -> Res<()> {
        sqlx::query(
            r#"
                INSERT INTO media (
                    uuid, user, relative_path, title, hashed_value, file_size, w, h
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8);
            "#,
        )
        .bind(format_uuid(self.uuid))
        .bind(format_uuid(self.user))
        .bind(&self.relative_path)
        .bind(&self.title)
        .bind(&self.hashed_value)
        .bind(self.file_size)
        .bind(self.w)
        .bind(self.h)
        .execute(conn)
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn user_total_size(pool: &SqlitePool, user: Uuid) -> Res<usize> {
        let row = sqlx::query("SELECT SUM(size) FROM media WHERE user = ?1;")
            .bind(format_uuid(user))
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;

        let size: i64 = row.get(0);
        Ok(size as usize)
    }

    pub async fn load(pool: &SqlitePool, uuid: Uuid) -> Res<Media> {
        sqlx::query_as("SELECT * FROM media WHERE uuid = ?1;")
            .bind(uuid)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Media item not found: {e}"))
    }

    pub async fn delete(pool: &SqlitePool, uuid: Uuid) -> Res<()> {
        sqlx::query("DELETE FROM media WHERE uuid = ?1;")
            .bind(format_uuid(uuid))
            .execute(pool)
            .await
            .map_err(|_| format!("Media item not found."))?;
        Ok(())
    }

    pub async fn update(
        pool: &SqlitePool,
        user: Uuid,
        uuid: Uuid,
        title: String,
        w: f32,
        h: f32,
    ) -> Res<()> {
        sqlx::query("UPDATE media SET title = ?1, w = ?2, h = ?3 WHERE uuid = ?4 AND user = ?5;")
            .bind(&title)
            .bind(w)
            .bind(h)
            .bind(format_uuid(uuid))
            .bind(format_uuid(user))
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn user_media(pool: &SqlitePool, user: Uuid) -> Res<Vec<Media>> {
        let results = sqlx::query_as("SELECT * FROM media WHERE user = ?1;")
            .bind(format_uuid(user))
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(results)
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
