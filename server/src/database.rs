use sqlx::sqlite::SqlitePool;

const DATABASE_FILE: &str = "database.db";
pub async fn connect(data_dir: &str) -> SqlitePool {
    SqlitePool::connect(format!("{}/{}", data_dir, DATABASE_FILE).as_str())
        .await
        .expect("Database pool creation failed.")
}
