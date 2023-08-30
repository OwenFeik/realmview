use std::fmt::Write;
use std::{fmt::Display, pin::Pin};

use actix_web::{
    body::BoxBody,
    error::{ErrorInternalServerError, ErrorUnprocessableEntity},
    http::StatusCode,
    FromRequest, HttpResponse, ResponseError,
};
use futures::Future;

use crate::models::User;

pub const COOKIE_NAME: &'static str = "session_key";

#[derive(Debug)]
struct LoginRedirect {
    path: String,
}

impl Display for LoginRedirect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Session not found. Redirecting.")
    }
}

impl ResponseError for LoginRedirect {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        let redirect = format!("/login?backurl={}", self.path);

        let mut buf = bytes::BytesMut::new();
        write!(&mut buf, "{}", self).ok();

        HttpResponse::build(StatusCode::SEE_OTHER)
            .insert_header(("location", redirect.as_str()))
            .body(format!("{}", self))
    }
}

fn login_redirect<T, S: ToString>(path: S) -> Result<T, actix_web::Error> {
    let redirect = LoginRedirect {
        path: path.to_string(),
    };
    Err(redirect.into())
}

async fn session_from_req(req: &actix_web::HttpRequest) -> Result<SessionOpt, actix_web::Error> {
    if let Some(cookie) = req.cookie(COOKIE_NAME) {
        let key = cookie.value();
        if let Some(pool) = super::POOL.get() {
            let session = match User::get_by_session(pool, key)
                .await
                .map_err(ErrorInternalServerError)?
            {
                Some(user) => SessionOpt::Some(Session {
                    key: key.to_string(),
                    user,
                }),
                None => SessionOpt::None,
            };
            Ok(session)
        } else {
            Err(ErrorInternalServerError("Pool not available."))
        }
    } else {
        Err(ErrorUnprocessableEntity("Missing cookie."))
    }
}

async fn session_or_redirect(req: &actix_web::HttpRequest) -> Result<Session, actix_web::Error> {
    match session_from_req(req).await {
        Ok(SessionOpt::Some(session)) => Ok(session),
        Ok(SessionOpt::None) | Err(_) => login_redirect(req.path()),
    }
}

#[derive(Debug)]
pub struct Session {
    pub key: String,
    pub user: User,
}

#[derive(Debug)]
pub enum SessionOpt {
    Some(Session),
    None,
}

impl FromRequest for Session {
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Session, Self::Error>>>>;

    fn from_request(
        req: &actix_web::HttpRequest,
        _payload: &mut actix_web::dev::Payload,
    ) -> Self::Future {
        let req = req.clone();
        Box::pin(async move { session_or_redirect(&req).await })
    }
}

impl FromRequest for User {
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<User, Self::Error>>>>;

    fn from_request(
        req: &actix_web::HttpRequest,
        _payload: &mut actix_web::dev::Payload,
    ) -> Self::Future {
        let req = req.clone();
        Box::pin(async move { session_or_redirect(&req).await.map(|s| s.user) })
    }
}

impl FromRequest for SessionOpt {
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<SessionOpt, Self::Error>>>>;

    fn from_request(
        req: &actix_web::HttpRequest,
        _payload: &mut actix_web::dev::Payload,
    ) -> Self::Future {
        let req = req.clone();
        Box::pin(async move {
            match session_from_req(&req).await {
                Err(_) => Ok(SessionOpt::None),
                ok => ok,
            }
        })
    }
}
