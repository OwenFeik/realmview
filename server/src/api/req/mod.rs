mod conn;
pub mod session;

use once_cell::sync::{Lazy, OnceCell};
use sqlx::SqlitePool;

pub static POOL: Lazy<OnceCell<SqlitePool>> = Lazy::new(OnceCell::new);

pub use conn::Conn;
