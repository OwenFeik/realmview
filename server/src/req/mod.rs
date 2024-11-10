mod conn;
pub mod session;

use actix_web::{http::StatusCode, HttpResponse};
use once_cell::sync::{Lazy, OnceCell};
use sqlx::SqlitePool;

static POOL: Lazy<OnceCell<SqlitePool>> = Lazy::new(OnceCell::new);

pub use conn::Pool;

pub fn set_pool(pool: SqlitePool) {
    POOL.set(pool).expect("Failed to set pool reference.");
}

pub fn e500<E>(error: E) -> actix_web::Error
where
    E: std::fmt::Debug + std::fmt::Display + 'static,
{
    actix_web::error::ErrorInternalServerError(error)
}

pub fn redirect(to: &str) -> HttpResponse {
    HttpResponse::build(StatusCode::SEE_OTHER)
        .insert_header(("location", to))
        .finish()
}
