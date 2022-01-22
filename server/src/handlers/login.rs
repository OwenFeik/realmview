use std::convert::Infallible;

use serde_derive::Deserialize;
use sqlx::sqlite::SqlitePool;
use warp::Filter;
use warp::hyper::StatusCode;

use super::models::User;
use super::{current_time, json_body, with_db};
use super::crypto::{check_password, generate_salt, from_hex_string, to_hex_string};
use super::response::{Binary, cookie_result};


#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String
}


fn decode_and_check_password(provided: &str, salt: &str, hashed_password: &str) -> anyhow::Result<bool> {
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


fn session_result(success: bool, message: &str, status: StatusCode, session_key: Option<&str>)
    -> Result<impl warp::Reply, Infallible>
{
    cookie_result(&Binary::new(message, success), status, "session_key", session_key)
}


async fn login(details: LoginRequest, pool: SqlitePool) -> Result<impl warp::Reply, Infallible> {
    let user = match User::get(&pool, details.username.as_str()).await {
        Ok(Some(u)) => u,
        Ok(None) => return session_result(false, "User does not exist.", StatusCode::OK, None),
        Err(_) => return session_result(false, "Database error.", StatusCode::INTERNAL_SERVER_ERROR, None)
    };

    match decode_and_check_password(details.password.as_str(), user.salt.as_str(), user.hashed_password.as_str()) {
        Ok(true) => (),
        Ok(false) => return session_result(false, "Incorrect password.", StatusCode::OK, None),
        Err(_) => return session_result(false, "Cryptography error.", StatusCode::INTERNAL_SERVER_ERROR, None)
    };

    let session_key = match create_user_session(&pool, &user).await {
        Ok(s) => s,
        Err(_) => return session_result(false, "Database error.", StatusCode::INTERNAL_SERVER_ERROR, None)
    };

    session_result(true, "Logged in.", StatusCode::OK, Some(session_key.as_str()))
}


pub fn filter(pool: SqlitePool) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("login")
        .and(warp::post())
        .and(json_body::<LoginRequest>())
        .and(with_db(pool))
        .and_then(login)
}
