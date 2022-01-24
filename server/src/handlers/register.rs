use std::convert::Infallible;

use serde_derive::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, Sqlite};
use warp::{Filter, http::StatusCode};

use super::{current_time, json_body, with_db};
use super::crypto::{Key, generate_salt, hash_password, to_hex_string};
use super::response::{as_result, Binary};


#[derive(Deserialize)]
struct RegistrationRequest {
    username: String,
    password: String
}


#[derive(Serialize)]
struct RegistrationResponse {
    message: String,
    recovery_key: String,
    success: bool,
    username: String
}

impl RegistrationResponse {
    fn new(recovery_key: String, username: String) -> RegistrationResponse {
        RegistrationResponse {
            message: String::from("Registration successful."),
            recovery_key,
            success: true,
            username
        }
    }
}


fn valid_username(username: &String) -> bool {
    username.len() >= 2 && username.len() <= 32
}


fn valid_password(password: &String) -> bool {
    password.len() >= 8
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
    created_time: u64
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


fn get_hex_strings(salt: &Key, hashed_password: &Key, recovery_key: &Key) -> anyhow::Result<(String, String, String)> {
    Ok((to_hex_string(salt)?, to_hex_string(hashed_password)?, to_hex_string(recovery_key)?))
}


fn generate_keys(password: &str) -> anyhow::Result<(String, String, String)> {
    let salt = generate_salt()?;
    let hashed_password = hash_password(&salt, password);
    let recovery_key = generate_salt()?;
    get_hex_strings(&salt, &hashed_password, &recovery_key)
}


async fn register(details: RegistrationRequest, pool: SqlitePool) -> Result<impl warp::Reply, Infallible> {
    if !valid_username(&details.username) {
        return Binary::result_failure("Invalid username.");
    }
    
    match username_taken(&pool, details.username.as_str()).await {
        Ok(true) => return Binary::result_failure("Username in use."),
        Ok(false) => (),
        Err(_) => return Binary::result_error("Database error when checking username availability.")
    };

    if !valid_password(&details.password) {
        return Binary::result_failure("Invalid password.");
    }

    let (s_salt, s_hpw, s_rkey) = match generate_keys(details.password.as_str()) {
        Ok(strings) => strings,
        Err(_) => return Binary::result_error("Cryptography error.")
    };

    let created_time = match current_time() {
        Ok(t) => t,
        Err(_) => return Binary::result_error("Server time issue.")
    };
    
    match register_user(
        &pool,
        details.username.as_str(),
        s_salt.as_str(),
        s_hpw.as_str(),
        s_rkey.as_str(),
        created_time
    ).await {
        Ok(_id) => as_result(&RegistrationResponse::new(s_rkey, details.username), StatusCode::OK),
        Err(_) => Binary::result_error("Database error on insertion.")
    }
}


pub fn filter(pool: SqlitePool) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("register")
        .and(warp::post())
        .and(json_body::<RegistrationRequest>())
        .and(with_db(pool))
        .and_then(register)
}
