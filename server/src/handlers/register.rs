use std::convert::Infallible;
use std::fmt::Write;
use std::num::NonZeroU32;

use ring::{pbkdf2, rand::{SecureRandom, SystemRandom}};
use serde_derive::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, Sqlite};
use warp::Filter;

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


fn generate_keys(password: &str) -> Option<(String, String, String)> {
    let salt = match generate_salt() {
        Some(s) => s,
        None => return None
    };
    
    let hashed_password = match hash_password(&salt, password) {
        Ok(h) => h,
        Err(_) => return None
    };

    let recovery_key = match generate_salt() {
        Some(s) => s,
        None => return None
    };

    match get_hex_strings(&salt, &hashed_password, &recovery_key) {
        Ok(strings) => Some(strings),
        Err(_) => None
    }
}


async fn register(details: Registration, pool: SqlitePool) -> Result<impl warp::Reply, Infallible> {
    if !valid_username(&details.username) {
        return response::Binary::reply_failure("Invalid username.");
    }
    
    match username_taken(&pool, details.username.as_str()).await {
        Ok(true) => return response::Binary::reply_failure("Username in use."),
        Ok(false) => (),
        Err(_) => return response::Binary::reply_error("Database error.")
    };

    if !valid_password(&details.password) {
        return response::Binary::reply_failure("Invalid password.");
    }

    let (ssalt, shpw, srkey) = match generate_keys(details.password.as_str()) {
        Some(strings) => strings,
        None => return response::Binary::reply_error("Cryptography error.")
    };

    let created_time = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => return response::Binary::reply_error("Server time issue.")
    };
    
    match register_user(
        &pool,
        details.username.as_str(),
        ssalt.as_str(),
        shpw.as_str(),
        srkey.as_str(),
        created_time
    ).await {
        Ok(_id) => response::Binary::reply_success("User registered."),
        Err(_) => response::Binary::reply_error("Database error.")
    }
}


pub fn filter(pool: SqlitePool) -> crate::handlers::ConfiguredFilter {
    warp::path!("register")
        .and(warp::post())
        .and(json_body::<Registration>())
        .and(with_db(pool))
        .and_then(register)
}
