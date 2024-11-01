use actix_web::{cookie::Cookie, http::StatusCode, HttpResponse, HttpResponseBuilder};

mod auth;
mod game;
mod media;
mod project;
mod register;
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

type Resp = Result<HttpResponse, actix_web::Error>;

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

fn session_resp(session: &str) -> HttpResponseBuilder {
    cookie_resp(crate::req::session::COOKIE_NAME, session)
}

fn resp_success(message: &str) -> HttpResponse {
    HttpResponse::Ok().json(body_success(message))
}

fn res_success(message: &str) -> Resp {
    Ok(resp_success(message))
}

fn resp_failure(message: &str) -> HttpResponse {
    HttpResponse::Ok().json(body_failure(message))
}

fn res_failure(message: &str) -> Resp {
    Ok(resp_failure(message))
}

fn resp_unproc(message: &str) -> HttpResponse {
    HttpResponse::UnprocessableEntity().json(body_failure(message))
}

fn res_unproc(message: &str) -> Resp {
    Ok(resp_unproc(message))
}

fn resp(message: &str, success: bool) -> HttpResponse {
    if success {
        resp_success(message)
    } else {
        resp_failure(message)
    }
}

fn res(message: &str, success: bool) -> Resp {
    Ok(resp(message, success))
}

fn resp_json(body: impl serde::Serialize) -> HttpResponse {
    HttpResponse::Ok().json(body)
}

fn res_json(body: impl serde::Serialize) -> Resp {
    Ok(resp_json(body))
}
