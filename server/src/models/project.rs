use sqlx::SqlitePool;

#[derive(sqlx::FromRow)]
pub struct Project {
    id: i64,
    user: i64,
    title: String,
}

impl Project {
    async fn new(pool: &SqlitePool, user: i64) -> anyhow::Result<Project> {
        sqlx::query_as("INSERT INTO projects (user, title) VALUES (?1, ?2) RETURNING *;")
            .bind(user)
            .bind("Untitled")
            .fetch_one(pool)
            .await
            .map_err(|_| anyhow::anyhow!("Database error."))
    }

    async fn load(pool: &SqlitePool, id: i64) -> anyhow::Result<Project> {
        let res = sqlx::query_as("SELECT * FROM projects WHERE id = ?1;")
            .bind(id)
            .fetch_optional(pool)
            .await;

        match res {
            Ok(Some(p)) => Ok(p),
            Ok(None) => Err(anyhow::anyhow!("Project not found.")),
            Err(_) => Err(anyhow::anyhow!("Database error.")),
        }
    }

    pub async fn get_or_create(
        pool: &SqlitePool,
        id: Option<i64>,
        user: i64,
    ) -> anyhow::Result<Project> {
        match id {
            Some(id) => Project::load(pool, id).await,
            None => Project::new(pool, user).await,
        }
    }
}
