use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use sqlx::{pool::PoolConnection, Sqlite, SqlitePool};
use tokio::sync::OnceCell;

use crate::utils::Res;

static DATA: Lazy<PathBuf> =
    Lazy::new(|| PathBuf::from(std::env::var("DATA_DIR").expect(super::USAGE)));

pub static CONTENT: Lazy<PathBuf> = Lazy::new(|| DATA.join("content"));
pub static SAVES: Lazy<PathBuf> = Lazy::new(|| DATA.join("saves"));

// For production we have a single sqlite pool, initialised once in a OnceCell
// and then cloned for future accesses.
#[cfg(not(test))]
static DATABASE: Lazy<OnceCell<SqlitePool>> = Lazy::new(OnceCell::new);

#[cfg(not(test))]
pub async fn initialise_database() -> Res<SqlitePool> {
    if DATABASE.initialized() {
        return Ok(DATABASE.get().unwrap().clone());
    }

    let database_url = std::env::var("DATABASE_URL").expect(super::USAGE);
    let pool = SqlitePool::connect(&database_url)
        .await
        .expect("Database pool creation failed.");

    DATABASE.set(pool).map_err(|e| e.to_string())?;
    Ok(DATABASE.get().unwrap().clone())
}

// For testing purposes we create a new database for each thread so that
// parallel tests don't interfere with each other by creating and deleting
// the same users, etc.
#[cfg(test)]
thread_local! {
    static DATABASE_INITIALISED: std::sync::atomic::AtomicBool = const { std::sync::atomic::AtomicBool::new(false) };
    static DATABASE: OnceCell<SqlitePool> = OnceCell::new();
}

#[cfg(test)]
pub async fn initialise_database() -> Res<SqlitePool> {
    if DATABASE_INITIALISED.with(|ab| ab.load(std::sync::atomic::Ordering::SeqCst)) {
        DATABASE.with(|cell| Ok(cell.get().unwrap().clone()))
    } else {
        let path = DATA.join(format!(
            "database-{}.db",
            std::thread::current().id().as_u64()
        ));
        println!("Database path: {}", path.display());

        tokio::fs::copy(DATA.join("database.db"), &path)
            .await
            .expect("Failed to copy database.");
        let database_url = format!("sqlite://{}", path.to_string_lossy());
        let pool = SqlitePool::connect(&database_url)
            .await
            .expect("Database pool creation failed.");

        DATABASE.with(|cell| match cell.set(pool.clone()) {
            Ok(_) => {
                DATABASE_INITIALISED.with(|ab| ab.store(true, std::sync::atomic::Ordering::SeqCst));
                Ok(pool)
            }
            Err(e) => Err(e.to_string()),
        })
    }
}

pub async fn database_connection() -> Res<PoolConnection<Sqlite>> {
    initialise_database()
        .await?
        .acquire()
        .await
        .map_err(|e| e.to_string())
}

/// Join a path with a relative path, that may start with a slash. If the
/// second argument starts with a slash, all leading slashes will be removed
/// before joining.
pub fn join_relative_path<S: AsRef<str>>(left: &Path, right: S) -> PathBuf {
    left.join(right.as_ref().trim_start_matches('/'))
}

pub async fn write_file<P: AsRef<Path>, D: AsRef<[u8]>>(path: P, data: D) -> Res<()> {
    if let Some(parent) = path.as_ref().parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create upload directory: {e}"))?;
    }

    tokio::fs::write(path, data)
        .await
        .map_err(|e| format!("Failed to write file: {e}"))
}
