use actix_web::{
    cookie::Cookie, error::ErrorInternalServerError, http::StatusCode, HttpResponse,
    HttpResponseBuilder,
};

mod auth;
mod project;
mod register;
pub mod req;

pub fn routes() -> actix_web::Scope {
    actix_web::web::scope("/api")
        .service(auth::routes())
        .service(project::routes())
        .service(register::routes())
}

#[derive(serde_derive::Serialize)]
struct Binary {
    message: String,
    success: bool,
}

fn body_success<T: ToString>(message: T) -> Binary {
    Binary {
        message: message.to_string(),
        success: true,
    }
}

fn body_failure<T: ToString>(message: T) -> Binary {
    Binary {
        message: message.to_string(),
        success: false,
    }
}

fn cookie_resp(cookie: &str, value: &str) -> HttpResponseBuilder {
    let mut builder = HttpResponse::build(StatusCode::OK);
    builder.cookie(Cookie::build(cookie, value).path("/").finish());
    builder
}

fn session_resp(session_key: &str) -> HttpResponseBuilder {
    cookie_resp(req::session::COOKIE_NAME, session_key)
}

fn resp_success(message: &str) -> HttpResponse {
    HttpResponse::Ok().json(body_success(message))
}

fn resp_failure(message: &str) -> HttpResponse {
    HttpResponse::Ok().json(body_failure(message))
}

fn resp(message: &str, success: bool) -> HttpResponse {
    if success {
        resp_success(message)
    } else {
        resp_failure(message)
    }
}

fn e500<E>(error: E) -> actix_web::Error
where
    E: std::fmt::Debug + std::fmt::Display + 'static,
{
    ErrorInternalServerError(error)
}
