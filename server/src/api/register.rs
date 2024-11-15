use actix_web::{error::ErrorInternalServerError, web};
use sqlx::SqlitePool;

use super::{res_json, Resp};
use crate::{
    crypto::{generate_key, hash_password, to_hex_string, Key},
    models::{User, UserAuth},
    utils::Res,
};

pub fn routes() -> actix_web::Scope {
    actix_web::web::scope("/register").default_service(web::post().to(register))
}

#[cfg_attr(test, derive(serde_derive::Serialize))]
#[derive(serde_derive::Deserialize)]
struct RegistrationRequest {
    username: String,
    password: String,
}

#[cfg_attr(test, derive(serde_derive::Deserialize))]
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

fn generate_keys(password: &str) -> Res<(Key, Key, Key)> {
    let salt = generate_key()?;
    let hashed_password = hash_password(&salt, password);
    let recovery_key = generate_key()?;
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

    RegistrationResponse::success(to_hex_string(&s_rkey), details.username.clone())
}

#[cfg(test)]
mod test {
    use actix_web::test;

    use super::{RegistrationRequest, RegistrationResponse};
    use crate::{api::Binary, crypto::from_hex_string, fs::initialise_database};

    #[actix_web::test]
    async fn test_register() {
        let db = initialise_database().await.unwrap();
        let app = test::init_service(
            actix_web::App::new()
                .app_data(actix_web::web::Data::new(db.clone()))
                .service(crate::api::routes()),
        )
        .await;

        // No request body, should be a failure.
        let req = test::TestRequest::post().uri("/api/register").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_client_error());

        // Invalid username, should be a failure.
        let req = test::TestRequest::post()
            .uri("/api/register")
            .set_json(RegistrationRequest {
                username: "!containsnonalnum".into(),
                password: "val1dpassword".into(),
            })
            .to_request();
        let resp: RegistrationResponse = test::call_and_read_body_json(&app, req).await;
        assert!(!resp.success);
        assert_eq!(resp.problem_field, Some("username".into()));

        // Invalid password, should be a failure.
        let req = test::TestRequest::post()
            .uri("/api/register")
            .set_json(RegistrationRequest {
                username: "valid".into(),
                password: "bad".into(),
            })
            .to_request();
        let resp: RegistrationResponse = test::call_and_read_body_json(&app, req).await;
        assert!(!resp.success);
        assert_eq!(resp.problem_field, Some("password".into()));

        // Valid username and password, should be a success.
        let req = test::TestRequest::post()
            .uri("/api/register")
            .set_json(RegistrationRequest {
                username: "valid".into(),
                password: "p4ssword".into(),
            })
            .to_request();
        let resp: RegistrationResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);
        assert!(resp
            .recovery_key
            .is_some_and(|key| from_hex_string(&key).is_ok()));
        assert_eq!(resp.username, Some("valid".into()));
        assert!(resp.problem_field.is_none());

        // Account already exists, should be a failure.
        let req = test::TestRequest::post()
            .uri("/api/register")
            .set_json(RegistrationRequest {
                username: "valid".into(),
                password: "p4ssword".into(),
            })
            .to_request();
        let resp: RegistrationResponse = test::call_and_read_body_json(&app, req).await;
        assert!(!resp.success);
        assert_eq!(resp.problem_field, Some("username".into()));

        // Logging into account we created should work.
        let req = test::TestRequest::post()
            .uri("/api/auth/login")
            .set_json(RegistrationRequest {
                username: "valid".into(),
                password: "p4ssword".into(),
            })
            .to_request();
        let resp: Binary = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);
    }
}
