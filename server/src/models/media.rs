use anyhow::anyhow;
use sqlx::{Row, SqlitePool};

use crate::crypto::random_hex_string;

#[derive(sqlx::FromRow)]
pub struct Media {
    pub id: i64,
    pub media_key: String,
    pub user: i64,
    pub relative_path: String,
    pub title: String,
    pub hashed_value: String,
    pub size: i64,
}

impl Media {
    const KEY_LENGTH: usize = 16;

    pub async fn create(
        pool: &SqlitePool,
        key: &str,
        user: i64,
        relative_path: &str,
        title: &str,
        hash: &str,
        size: i64,
    ) -> anyhow::Result<Media> {
        sqlx::query_as(
            r#"
                INSERT INTO media (
                    media_key, user, relative_path, title, hashed_value, size
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6) RETURNING *;
            "#,
        )
        .bind(key)
        .bind(user)
        .bind(relative_path)
        .bind(title)
        .bind(hash)
        .bind(size)
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow!("Database error: {e}"))
    }

    pub async fn user_total_size(pool: &SqlitePool, user: i64) -> anyhow::Result<usize> {
        let row = sqlx::query("SELECT SUM(size) FROM media WHERE user = ?1;")
            .bind(user)
            .fetch_one(pool)
            .await
            .map_err(|e| anyhow!(e))?;

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

    pub fn generate_key() -> anyhow::Result<String> {
        let key = random_hex_string(Media::KEY_LENGTH)?;

        // Always generate a key with a positive value.
        // 63 / 64 keys generated should already be positive, so this is
        // unlikely to recurse very far.
        if Self::key_to_id(&key)? <= 0 {
            Self::generate_key()
        } else {
            Ok(key)
        }
    }

    pub fn key_to_id(key: &str) -> anyhow::Result<i64> {
        if key.len() != Media::KEY_LENGTH {
            return Err(anyhow!("Invalid media key."));
        }

        let mut raw = [0; 8];
        for (i, r) in raw.iter_mut().enumerate() {
            let j = i * 2;
            if let Ok(b) = u8::from_str_radix(&key[j..j + 2], 16) {
                *r = b;
            } else {
                return Err(anyhow!("Invalid hexadecimal."));
            }
        }

        Ok(i64::from_be_bytes(raw))
    }

    pub fn id_to_key(id: i64) -> String {
        format!("{:016X}", id)
    }
}
