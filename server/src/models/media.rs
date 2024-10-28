use anyhow::anyhow;
use sqlx::{Row, SqlitePool};

#[derive(sqlx::FromRow)]
pub struct Media {
    pub id: i64,
    pub media_key: String,
    pub user: i64,
    pub relative_path: String,
    pub title: String,
    pub hashed_value: String,
    pub size: i64,
    pub w: f32,
    pub h: f32,
}

impl Media {
    const KEY_LENGTH: usize = 16;
    const DEFAULT_SIZE: f32 = 1.0;

    pub fn new(
        key: String,
        user: i64,
        relative_path: String,
        title: String,
        hash: String,
        size: i64,
    ) -> Self {
        Self {
            id: 0,
            media_key: key,
            user,
            relative_path,
            title,
            hashed_value: hash,
            size,
            w: Self::DEFAULT_SIZE,
            h: Self::DEFAULT_SIZE,
        }
    }

    pub async fn create(&self, pool: &SqlitePool) -> anyhow::Result<()> {
        sqlx::query(
            r#"
                INSERT INTO media (
                    media_key, user, relative_path, title, hashed_value, size, w, h
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8);
            "#,
        )
        .bind(&self.media_key)
        .bind(self.user)
        .bind(&self.relative_path)
        .bind(&self.title)
        .bind(&self.hashed_value)
        .bind(self.size)
        .bind(self.w)
        .bind(self.h)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn user_total_size(pool: &SqlitePool, user: i64) -> anyhow::Result<usize> {
        let row = sqlx::query("SELECT SUM(size) FROM media WHERE user = ?1;")
            .bind(user)
            .fetch_one(pool)
            .await?;

        let size: i64 = row.get(0);
        Ok(size as usize)
    }

    pub async fn load(pool: &SqlitePool, key: &str) -> anyhow::Result<Media> {
        sqlx::query_as("SELECT * FROM media WHERE media_key = ?1;")
            .bind(key)
            .fetch_one(pool)
            .await
            .map_err(|e| anyhow!("Media item not found: {e}"))
    }

    pub async fn delete(pool: &SqlitePool, key: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM media WHERE media_key = ?1;")
            .bind(key)
            .execute(pool)
            .await
            .map_err(|_| anyhow!("Media item not found."))?;
        Ok(())
    }

    pub async fn update(
        pool: &SqlitePool,
        user: i64,
        key: String,
        title: String,
        w: f32,
        h: f32,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE media SET title = ?1, w = ?2, h = ?3 WHERE media_key = ?4 AND user = ?5;",
        )
        .bind(&title)
        .bind(w)
        .bind(h)
        .bind(key)
        .bind(user)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn user_media(pool: &SqlitePool, user_id: i64) -> anyhow::Result<Vec<Media>> {
        let results = sqlx::query_as("SELECT * FROM media WHERE user = ?1;")
            .bind(user_id)
            .fetch_all(pool)
            .await?;
        Ok(results)
    }
}
