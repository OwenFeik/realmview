use actix_web::{error::ErrorInternalServerError, web, HttpResponse};
use sqlx::SqlitePool;

use super::req::session::{Session, SessionOpt};
use super::{body_failure, body_success, resp, session_resp, Res};
use crate::{
    crypto::{check_password, from_hex_string},
    models::{User, UserSession},
};

pub fn routes() -> actix_web::Scope {
    actix_web::web::scope("/auth")
        .route("/login", web::post().to(login))
        .route("/test", web::post().to(test))
        .route("/logout", web::post().to(logout))
}

fn decode_and_check_password(
    provided: &str,
    salt: &str,
    hashed_password: &str,
) -> anyhow::Result<bool> {
    let salt = from_hex_string(salt)?;
    let hashed_password = from_hex_string(hashed_password)?;
    Ok(check_password(provided, &salt, &hashed_password))
}

#[derive(serde_derive::Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

async fn login(pool: web::Data<SqlitePool>, req: web::Json<LoginRequest>) -> Res {
    let Some(user) = User::get(&pool, req.username.as_str())
        .await
        .map_err(ErrorInternalServerError)? else
    {
        return Ok(session_resp("").json(body_failure("User does not exist.")));
    };

    if !decode_and_check_password(
        req.password.as_str(),
        user.salt.as_str(),
        user.hashed_password.as_str(),
    )
    .map_err(ErrorInternalServerError)?
    {
        return Ok(session_resp("").json(body_failure("Incorrect password.")));
    };

    let session_key = UserSession::create(&pool, &user)
        .await
        .map_err(ErrorInternalServerError)?;

    Ok(session_resp(&session_key).json(body_success("Logged in.")))
}

async fn test(session: SessionOpt) -> HttpResponse {
    let success = !matches!(session, SessionOpt::None);
    let message = if success {
        "Session valid."
    } else {
        "Invalid session."
    };

    resp(message, success)
}

async fn logout(pool: web::Data<SqlitePool>, session: SessionOpt) -> HttpResponse {
    if let SessionOpt::Some(Session { key, .. }) = session {
        UserSession::end(&pool, &key).await.ok();
    }

    session_resp("").json(body_success("Logged out."))
}
