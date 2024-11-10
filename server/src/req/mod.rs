mod conn;
pub mod session;

use actix_web::{http::StatusCode, HttpResponse};
pub use conn::Pool;

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
