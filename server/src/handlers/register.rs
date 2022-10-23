use std::convert::Infallible;

use serde_derive::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, Sqlite};
use warp::{http::StatusCode, Filter};

use super::response::{as_result, Binary};
use super::{current_time, json_body, with_db};
use crate::crypto::{generate_salt, hash_password, to_hex_string, Key};

#[derive(Deserialize)]
struct RegistrationRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct RegistrationResponse {
    message: String,
    recovery_key: Option<String>,
    success: bool,
    username: Option<String>,
    problem_field: Option<String>,
}

impl RegistrationResponse {
    fn success(
        recovery_key: String,
        username: String,
    ) -> Result<warp::reply::WithStatus<warp::reply::Json>, Infallible> {
        as_result(
            &RegistrationResponse {
                message: String::from("Registration successful."),
                recovery_key: Some(recovery_key),
                success: true,
                username: Some(username),
                problem_field: None,
            },
            StatusCode::OK,
        )
    }

    fn failure(
        message: &str,
        problem_field: &str,
    ) -> Result<warp::reply::WithStatus<warp::reply::Json>, Infallible> {
        as_result(
            &RegistrationResponse {
                message: message.to_string(),
                recovery_key: None,
                success: false,
                username: None,
                problem_field: Some(problem_field.to_string()),
            },
            StatusCode::OK,
        )
    }
}

// Usernames are 4-32 alphanumeric characters
fn valid_username(username: &str) -> bool {
    username.chars().all(char::is_alphanumeric) && username.len() >= 4 && username.len() <= 32
}

// Passwords are 8 or more characters with at least one letter and at least one
// number
fn valid_password(password: &str) -> bool {
    password.chars().any(char::is_numeric)
        && password.chars().any(char::is_alphabetic)
        && password.len() >= 8
}

async fn username_taken(pool: &SqlitePool, username: &str) -> anyhow::Result<bool> {
    let row = sqlx::query::<Sqlite>("SELECT id FROM users WHERE username = ?1;")
        .bind(username)
        .fetch_optional(pool)
        .await?;

    Ok(row.is_some())
}

async fn register_user(
    pool: &SqlitePool,
    username: &str,
    salt: &str,
    hashed_password: &str,
    recovery_key: &str,
    created_time: u64,
) -> anyhow::Result<i64> {
    let id = sqlx::query(
        "INSERT INTO users (username, salt, hashed_password, recovery_key, created_time) VALUES (?1, ?2, ?3, ?4, ?5);"
    )
        .bind(username)
        .bind(salt)
        .bind(hashed_password)
        .bind(recovery_key)
        .bind(created_time as i64)
        .execute(&mut pool.acquire().await?)
        .await?
        .last_insert_rowid();

    Ok(id)
}

fn get_hex_strings(
    salt: &Key,
    hashed_password: &Key,
    recovery_key: &Key,
) -> anyhow::Result<(String, String, String)> {
    Ok((
        to_hex_string(salt)?,
        to_hex_string(hashed_password)?,
        to_hex_string(recovery_key)?,
    ))
}

fn generate_keys(password: &str) -> anyhow::Result<(String, String, String)> {
    let salt = generate_salt()?;
    let hashed_password = hash_password(&salt, password);
    let recovery_key = generate_salt()?;
    get_hex_strings(&salt, &hashed_password, &recovery_key)
}

async fn register(
    details: RegistrationRequest,
    pool: SqlitePool,
) -> Result<impl warp::Reply, Infallible> {
    if !valid_username(&details.username) {
        return RegistrationResponse::failure("Invalid username.", "username");
    }

    match username_taken(&pool, details.username.as_str()).await {
        Ok(true) => return RegistrationResponse::failure("Username in use.", "username"),
        Ok(false) => (),
        Err(_) => {
            return Binary::result_error("Database error when checking username availability.")
        }
    };

    if !valid_password(&details.password) {
        return RegistrationResponse::failure("Invalid password.", "password");
    }

    let (s_salt, s_hpw, s_rkey) = match generate_keys(details.password.as_str()) {
        Ok(strings) => strings,
        Err(_) => return Binary::result_error("Cryptography error."),
    };

    let created_time = match current_time() {
        Ok(t) => t,
        Err(_) => return Binary::result_error("Server time issue."),
    };

    match register_user(
        &pool,
        details.username.as_str(),
        s_salt.as_str(),
        s_hpw.as_str(),
        s_rkey.as_str(),
        created_time,
    )
    .await
    {
        Ok(_id) => RegistrationResponse::success(s_rkey, details.username),
        Err(_) => Binary::result_error("Database error on insertion."),
    }
}

pub fn filter(
    pool: SqlitePool,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("register")
        .and(warp::post())
        .and(json_body::<RegistrationRequest>())
        .and(with_db(pool))
        .and_then(register)
}
