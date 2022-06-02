#[derive(sqlx::FromRow)]
pub struct Media {
    pub id: i64,
    pub user: i64,
    pub relative_path: String,
    pub title: String,
    pub hashed_value: String,
}

impl Media {
    pub async fn load(pool: &sqlx::SqlitePool, id: i64) -> anyhow::Result<Media> {
        sqlx::query_as("SELECT * FROM media WHERE id = ?1;")
            .bind(id)
            .fetch_one(pool)
            .await
            .map_err(|e| anyhow::anyhow!("Media item not found: {e}"))
    }
}
