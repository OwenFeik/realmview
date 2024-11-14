use actix_web::{error::ErrorInternalServerError, web, HttpResponse};
use sqlx::SqlitePool;

use super::{body_failure, body_success, resp, session_resp, Resp};
use crate::crypto::Key;
use crate::models::UserAuth;
use crate::req::session::{Session, SessionOpt};
use crate::utils::Res;
use crate::{crypto::check_password, models::UserSession};

pub fn routes() -> actix_web::Scope {
    actix_web::web::scope("/auth")
        .route("/login", web::post().to(login))
        .route("/test", web::post().to(test))
        .route("/logout", web::post().to(logout))
}

fn decode_and_check_password(provided: &str, salt: &Key, hashed_password: &Key) -> Res<bool> {
    Ok(check_password(provided, salt, hashed_password))
}

#[derive(serde_derive::Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

async fn login(pool: web::Data<SqlitePool>, req: web::Json<LoginRequest>) -> Resp {
    let user = match UserAuth::get_by_username(&pool, req.username.as_str()).await {
        Ok(user) => user,
        Err(e) => return Ok(session_resp("").json(body_failure(e))),
    };

    if !decode_and_check_password(req.password.as_str(), &user.salt, &user.hashed_password)
        .map_err(ErrorInternalServerError)?
    {
        return Ok(session_resp("").json(body_failure("Incorrect password.")));
    };

    let session = UserSession::create(&pool, &user)
        .await
        .map_err(ErrorInternalServerError)?;

    Ok(session_resp(&session.simple().to_string()).json(body_success("Logged in.")))
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
