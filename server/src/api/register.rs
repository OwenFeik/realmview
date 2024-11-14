use actix_web::{error::ErrorInternalServerError, web};
use sqlx::SqlitePool;

use super::{res_json, Resp};
use crate::{
    crypto::{generate_salt, hash_password, to_hex_string, Key},
    models::{User, UserAuth},
    utils::Res,
};

pub fn routes() -> actix_web::Scope {
    actix_web::web::scope("/register").default_service(web::post().to(register))
}

#[derive(serde_derive::Deserialize)]
struct RegistrationRequest {
    username: String,
    password: String,
}

#[derive(serde_derive::Serialize)]
struct RegistrationResponse {
    message: String,
    recovery_key: Option<String>,
    success: bool,
    username: Option<String>,
    problem_field: Option<String>,
}

impl RegistrationResponse {
    fn success(recovery_key: String, username: String) -> Resp {
        res_json(RegistrationResponse {
            message: String::from("Registration successful."),
            recovery_key: Some(recovery_key),
            success: true,
            username: Some(username),
            problem_field: None,
        })
    }

    fn failure(message: &str, problem_field: &str) -> Resp {
        res_json(RegistrationResponse {
            message: message.to_string(),
            recovery_key: None,
            success: false,
            username: None,
            problem_field: Some(problem_field.to_string()),
        })
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

fn get_hex_strings(
    salt: &Key,
    hashed_password: &Key,
    recovery_key: &Key,
) -> Res<(String, String, String)> {
    Ok((
        to_hex_string(salt)?,
        to_hex_string(hashed_password)?,
        to_hex_string(recovery_key)?,
    ))
}

fn generate_keys(password: &str) -> Res<(Key, Key, Key)> {
    let salt = generate_salt()?;
    let hashed_password = hash_password(&salt, password);
    let recovery_key = generate_salt()?;
    Ok((salt, hashed_password, recovery_key))
}

async fn register(pool: web::Data<SqlitePool>, details: web::Json<RegistrationRequest>) -> Resp {
    if !valid_username(&details.username) {
        return RegistrationResponse::failure("Invalid username.", "username");
    }

    let conn = &mut pool.acquire().await.map_err(ErrorInternalServerError)?;
    if User::username_taken(conn, details.username.as_str())
        .await
        .map_err(ErrorInternalServerError)?
    {
        return RegistrationResponse::failure("Username in use.", "username");
    };

    if !valid_password(&details.password) {
        return RegistrationResponse::failure("Invalid password.", "password");
    }

    let (s_salt, s_hpw, s_rkey) =
        generate_keys(details.password.as_str()).map_err(ErrorInternalServerError)?;

    UserAuth::register(conn, details.username.as_str(), &s_salt, &s_hpw, &s_rkey)
        .await
        .map_err(ErrorInternalServerError)?;

    RegistrationResponse::success(
        to_hex_string(&s_rkey).map_err(ErrorInternalServerError)?,
        details.username.clone(),
    )
}
