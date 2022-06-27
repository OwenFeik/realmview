use crate::crypto::random_hex_string;

#[derive(sqlx::FromRow)]
pub struct Media {
    pub id: i64,
    pub media_key: String,
    pub user: i64,
    pub relative_path: String,
    pub title: String,
    pub hashed_value: String,
}

impl Media {
    const KEY_LENGTH: usize = 16;

    pub async fn create(pool: &sqlx::SqlitePool, key: &str, user: i64, relative_path: &str, title: &str, hash: &str) -> anyhow::Result<Media> {
        sqlx::query_as(
            "INSERT INTO media (media_key, user, relative_path, title, hashed_value) VALUES (?1, ?2, ?3, ?4, ?5) RETURNING *;"
        )
            .bind(key)
            .bind(user)
            .bind(relative_path)
            .bind(title)
            .bind(hash)
            .fetch_one(pool)
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {e}"))
    }

    pub async fn load(pool: &sqlx::SqlitePool, key: &str) -> anyhow::Result<Media> {
        sqlx::query_as("SELECT * FROM media WHERE media_key = ?1;")
            .bind(key)
            .fetch_one(pool)
            .await
            .map_err(|e| anyhow::anyhow!("Media item not found: {e}"))
    }

    pub fn generate_key() -> anyhow::Result<String> {
        let key = random_hex_string(Media::KEY_LENGTH)?;
        
        // Always generate a key with a positive value.
        // 63 / 64 keys generated should already be positive, so this is
        // unlikely to recurse very far.
        if Self::key_to_id(&key)? <= 0 {
            Self::generate_key()
        }
        else {
            Ok(key)
        }
    }

    pub fn key_to_id(key: &str) -> anyhow::Result<i64> {
        if key.len() != Media::KEY_LENGTH {
            return Err(anyhow::anyhow!("Invalid media key."));
        }
    
        let mut raw = [0; 8];
        for i in 0..8 {
            let j = i * 2;
            if let Ok(b) = u8::from_str_radix(&key[j..j + 2], 16) {
                raw[i] = b;
            }
            else {
                return Err(anyhow::anyhow!("Invalid hexadecimal."));
            }
        }
    
        Ok(i64::from_be_bytes(raw))
    }
    
    pub fn id_to_key(id: i64) -> String {
        format!("{:16X}", id)    
    }    
}
