use std::convert::Infallible;
use std::fmt::Write;
use std::num::NonZeroU32;

use ring::{pbkdf2, rand::{SecureRandom, SystemRandom}};
use serde_derive::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool};
use sqlx::Sqlite;
use warp::{Filter, http::StatusCode};

use crate::handlers::common::*;


#[derive(Deserialize, Serialize)]
struct Registration {
    username: String,
    password: String
}


const KEY_LENGTH: usize = ring::digest::SHA512_OUTPUT_LEN;
type Key = [u8; KEY_LENGTH];


fn generate_salt() -> Option<Key> {
    let mut bytes = [0u8; KEY_LENGTH];
    let rng = SystemRandom::new();
    match rng.fill(&mut bytes) {
        Ok(()) => Some(bytes),
        Err(_) => None 
    }
}


const ITERATIONS: u32 = 10_000;
fn hash_password(salt: &[u8], password: &str) -> anyhow::Result<Key> {
    let mut hashed = [0u8; KEY_LENGTH];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        NonZeroU32::new(ITERATIONS).unwrap(),
        &salt,
        password.as_bytes(),
        &mut hashed
    );

    Ok(hashed)
}


fn to_hex_string(key: &Key) -> anyhow::Result<String> {
    let mut s = String::with_capacity(KEY_LENGTH * 2);
    for byte in *key {
        write!(s, "{:02X}", byte)?;
    }

    Ok(s)
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

    Ok(row.is_none())
}


async fn register_user(
    pool: &SqlitePool,
    username: &str,
    salt: &str,
    hashed_password: &str,
    recovery_key: &str
) -> anyhow::Result<i64> {
    let id = sqlx::query("INSERT INTO users (username, salt, hashed_password, recovery_key) VALUES (?1, ?2, ?3, ?4);")
        .bind(username)
        .bind(salt)
        .bind(hashed_password)
        .bind(recovery_key)
        .execute(&mut pool.acquire().await?)
        .await?
        .last_insert_rowid();

    Ok(id)
}


fn get_hex_strings(salt: &Key, hashed_password: &Key, recovery_key: &Key) -> anyhow::Result<(String, String, String)> {
    Ok((to_hex_string(salt)?, to_hex_string(hashed_password)?, to_hex_string(recovery_key)?))
}


async fn register(details: Registration, pool: SqlitePool) -> Result<impl warp::Reply, Infallible> {
    if !valid_username(&details.username) {
        return Ok(StatusCode::OK);
    }
    
    match username_taken(&pool, details.username.as_str()).await {
        Ok(true) => return Ok(StatusCode::OK),
        Ok(false) => (),
        Err(_) => return Ok(StatusCode::INTERNAL_SERVER_ERROR) 
    };

    if !valid_password(&details.password) {
        return Ok(StatusCode::OK);
    }

    let salt = match generate_salt() {
        Some(s) => s,
        None => return Ok(StatusCode::INTERNAL_SERVER_ERROR)
    };
    
    let hashed_password = match hash_password(&salt, details.password.as_str()) {
        Ok(h) => h,
        Err(_) => return Ok(StatusCode::INTERNAL_SERVER_ERROR)
    };

    let recovery_key = match generate_salt() {
        Some(s) => s,
        None => return Ok(StatusCode::INTERNAL_SERVER_ERROR)
    };

    let (ssalt, shpw, srkey) = match get_hex_strings(&salt, &hashed_password, &recovery_key) {
        Ok(strings) => strings,
        Err(_) => return Ok(StatusCode::INTERNAL_SERVER_ERROR)
    };

    match register_user(&pool, details.username.as_str(), ssalt.as_str(), shpw.as_str(), srkey.as_str()).await {
        Ok(_id) => Ok(StatusCode::OK),
        Err(_) => Ok(StatusCode::INTERNAL_SERVER_ERROR)
    }
}


pub fn filter(pool: SqlitePool) -> crate::handlers::ConfiguredFilter {
    warp::path!("register")
        .and(warp::post())
        .and(json_body::<Registration>())
        .and(with_db(pool))
        .and_then(register)
}
