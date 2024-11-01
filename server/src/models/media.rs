use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use super::{format_uuid, Media};
use crate::utils::{generate_uuid, Res};

impl Media {
    const KEY_LENGTH: usize = 16;
    const DEFAULT_SIZE: f32 = 1.0;

    pub fn new(user: Uuid, relative_path: String, title: String, hash: String, size: i64) -> Self {
        Self {
            uuid: generate_uuid(),
            user,
            relative_path,
            title,
            hashed_value: hash,
            file_size: size,
            w: Self::DEFAULT_SIZE,
            h: Self::DEFAULT_SIZE,
        }
    }

    pub async fn create(&self, pool: &SqlitePool) -> Res<()> {
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
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn user_total_size(pool: &SqlitePool, user: i64) -> Res<usize> {
        let row = sqlx::query("SELECT SUM(size) FROM media WHERE user = ?1;")
            .bind(user)
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
}
