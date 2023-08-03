mod conn;
pub mod session;

use once_cell::sync::{Lazy, OnceCell};
use sqlx::SqlitePool;

static POOL: Lazy<OnceCell<SqlitePool>> = Lazy::new(OnceCell::new);

pub use conn::Conn;

pub fn set_pool(pool: SqlitePool) {
    POOL.set(pool).expect("Failed to set pool reference.");
}
