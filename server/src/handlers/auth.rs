use std::convert::Infallible;

use serde_derive::Deserialize;
use sqlx::SqlitePool;
use warp::hyper::StatusCode;
use warp::Filter;

use super::response::{cookie_result, Binary};
use super::{json_body, parse_cookie, with_db, with_session};
use crate::crypto::{check_password, from_hex_string, generate_salt, to_hex_string};
use crate::models::User;
use crate::utils::current_time;

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

pub fn filter(
    pool: SqlitePool,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let logout = warp::path("logout")
        .and(warp::post())
        .and(with_db(pool.clone()))
        .and(with_session())
        .and_then(logout);

    let login = warp::path("login")
        .and(warp::post())
        .and(json_body::<LoginRequest>())
        .and(with_db(pool.clone()))
        .and_then(login);

    let test = warp::path("test")
        .and(warp::post())
        .and(with_db(pool))
        .and(warp::header::optional("Cookie"))
        .and_then(test_session);

    warp::path("auth").and(login.or(logout).or(test))
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

async fn create_user_session(pool: &SqlitePool, user: &User) -> anyhow::Result<String> {
    let session_key = to_hex_string(&generate_salt()?)?;

    sqlx::query("INSERT INTO user_sessions (user, session_key, start_time) VALUES (?1, ?2, ?3);")
        .bind(user.id)
        .bind(session_key.as_str())
        .bind(current_time()? as i64)
        .execute(pool)
        .await?;

    Ok(session_key)
}

fn session_result(
    success: bool,
    message: &str,
    status: StatusCode,
    session_key: Option<&str>,
) -> Result<impl warp::Reply, Infallible> {
    cookie_result(
        &Binary::new(message, success),
        status,
        "session_key",
        session_key,
    )
}

async fn login(details: LoginRequest, pool: SqlitePool) -> Result<impl warp::Reply, Infallible> {
    let user = match User::get(&pool, details.username.as_str()).await {
        Ok(Some(u)) => u,
        Ok(None) => return session_result(false, "User does not exist.", StatusCode::OK, None),
        Err(_) => {
            return session_result(
                false,
                "Database error.",
                StatusCode::INTERNAL_SERVER_ERROR,
                None,
            )
        }
    };

    match decode_and_check_password(
        details.password.as_str(),
        user.salt.as_str(),
        user.hashed_password.as_str(),
    ) {
        Ok(true) => (),
        Ok(false) => return session_result(false, "Incorrect password.", StatusCode::OK, None),
        Err(_) => {
            return session_result(
                false,
                "Cryptography error.",
                StatusCode::INTERNAL_SERVER_ERROR,
                None,
            )
        }
    };

    let session_key = match create_user_session(&pool, &user).await {
        Ok(s) => s,
        Err(_) => {
            return session_result(
                false,
                "Database error.",
                StatusCode::INTERNAL_SERVER_ERROR,
                None,
            )
        }
    };

    session_result(
        true,
        "Logged in.",
        StatusCode::OK,
        Some(session_key.as_str()),
    )
}

async fn end_session(pool: &SqlitePool, session_key: &str) -> anyhow::Result<bool> {
    let rows_affected =
        sqlx::query("UPDATE user_sessions SET end_time = ?1 WHERE session_key = ?2")
            .bind(current_time()? as i64)
            .bind(session_key)
            .execute(pool)
            .await?
            .rows_affected();

    Ok(rows_affected > 0)
}

async fn logout(pool: SqlitePool, session_key: String) -> Result<impl warp::Reply, Infallible> {
    end_session(&pool, session_key.as_str()).await.ok();
    session_result(true, "Logged out.", StatusCode::OK, None)
}

async fn test_session(
    pool: SqlitePool,
    cookie: Option<String>,
) -> Result<impl warp::Reply, Infallible> {
    let (session_key, message) = if let Some(cookies) = cookie {
        if let Some(session_key) = parse_cookie(cookies, "session_key") {
            match User::get_by_session(&pool, &session_key).await {
                Ok(Some(_user)) => (Some(session_key), "Session valid."),
                Ok(None) => (None, "Invalid session."),
                Err(_) => (None, "Database error."),
            }
        } else {
            (None, "No session key.")
        }
    } else {
        (None, "No cookie.")
    };

    session_result(
        session_key.is_some(),
        message,
        StatusCode::OK,
        session_key.as_deref(),
    )
}
