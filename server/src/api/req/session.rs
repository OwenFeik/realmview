use std::pin::Pin;

use actix_web::{
    error::{ErrorForbidden, ErrorInternalServerError, ErrorUnprocessableEntity},
    FromRequest,
};
use futures::Future;

use crate::models::User;

pub const COOKIE_NAME: &str = "session_key";

async fn session_from_req(req: &actix_web::HttpRequest) -> Result<SessionOpt, actix_web::Error> {
    if let Some(cookie) = req.cookie(COOKIE_NAME) {
        let key = cookie.value();
        if let Some(Some(pool)) = super::POOL.get() {
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
        Box::pin(async move {
            match session_from_req(&req).await? {
                SessionOpt::Some(session) => Ok(session),
                SessionOpt::None => Err(ErrorForbidden("Session not found.")),
            }
        })
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
        Box::pin(async move {
            match session_from_req(&req).await? {
                SessionOpt::Some(session) => Ok(session.user),
                SessionOpt::None => Err(ErrorForbidden("Session not found.")),
            }
        })
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
        Box::pin(async move { session_from_req(&req).await })
    }
}
