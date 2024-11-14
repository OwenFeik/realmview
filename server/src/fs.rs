use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use sqlx::SqlitePool;
use tokio::sync::OnceCell;

use crate::utils::Res;

static DATA: Lazy<PathBuf> =
    Lazy::new(|| PathBuf::from(std::env::var("DATA_DIR").expect(super::USAGE)));

pub static CONTENT: Lazy<PathBuf> = Lazy::new(|| DATA.join("content"));
pub static SAVES: Lazy<PathBuf> = Lazy::new(|| DATA.join("saves"));

static DATABASE: Lazy<OnceCell<SqlitePool>> = Lazy::new(OnceCell::new);

pub async fn initialise_database() -> Res<SqlitePool> {
    let pool = SqlitePool::connect(&std::env::var("DATABASE_URL").expect(super::USAGE))
        .await
        .expect("Database pool creation failed.");

    DATABASE.set(pool).map_err(|e| e.to_string())?;
    Ok(database())
}

pub fn database() -> SqlitePool {
    DATABASE.get().expect("Database not initialised").clone()
}

/// Join a path with a relative path, that may start with a slash. If the
/// second argument starts with a slash, all leading slashes will be removed
/// before joining.
pub fn join_relative_path<S: AsRef<str>>(left: &Path, right: S) -> PathBuf {
    left.join(right.as_ref().trim_start_matches('/'))
}
