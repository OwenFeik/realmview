use actix_web::{
    cookie::Cookie, error::ErrorInternalServerError, http::StatusCode, HttpResponse,
    HttpResponseBuilder,
};
use sqlx::SqlitePool;

mod auth;
mod game;
mod media;
mod project;
mod register;
mod req;
mod scene;
mod upload;

pub fn routes() -> actix_web::Scope {
    actix_web::web::scope("/api")
        .service(auth::routes())
        .service(game::routes())
        .service(scene::routes())
        .service(project::routes())
        .service(media::routes())
        .service(register::routes())
        .service(upload::routes())
}

pub fn set_pool(pool: SqlitePool) {
    req::POOL.set(pool).expect("Failed to set pool reference.");
}

type Res = Result<HttpResponse, actix_web::Error>;

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

fn res_success(message: &str) -> Res {
    Ok(resp_success(message))
}

fn resp_failure(message: &str) -> HttpResponse {
    HttpResponse::Ok().json(body_failure(message))
}

fn res_failure(message: &str) -> Res {
    Ok(resp_failure(message))
}

fn resp_unproc(message: &str) -> HttpResponse {
    HttpResponse::UnprocessableEntity().json(body_failure(message))
}

fn res_unproc(message: &str) -> Res {
    Ok(resp_unproc(message))
}

fn resp(message: &str, success: bool) -> HttpResponse {
    if success {
        resp_success(message)
    } else {
        resp_failure(message)
    }
}

fn res(message: &str, success: bool) -> Res {
    Ok(resp(message, success))
}

fn resp_json(body: impl serde::Serialize) -> HttpResponse {
    HttpResponse::Ok().json(body)
}

fn res_json(body: impl serde::Serialize) -> Res {
    Ok(resp_json(body))
}

fn e500<E>(error: E) -> actix_web::Error
where
    E: std::fmt::Debug + std::fmt::Display + 'static,
{
    ErrorInternalServerError(error)
}
