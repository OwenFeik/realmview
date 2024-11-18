use actix_web::{error::ErrorInternalServerError, web, HttpResponse};
use sqlx::SqlitePool;

use super::{body_failure, body_success, resp, session_resp, Resp};
use crate::crypto::Key;
use crate::models::UserAuth;
use crate::req::session::SessionOpt;
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

#[cfg_attr(test, derive(serde_derive::Serialize))]
#[derive(serde_derive::Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

async fn login(pool: web::Data<SqlitePool>, req: web::Json<LoginRequest>) -> Resp {
    let conn = &mut pool.acquire().await.map_err(ErrorInternalServerError)?;
    let user = match UserAuth::get_by_username(conn, req.username.as_str()).await {
        Ok(user) => user,
        Err(e) => return Ok(session_resp("").json(body_failure(e))),
    };

    if !decode_and_check_password(req.password.as_str(), &user.salt, &user.hashed_password)
        .map_err(ErrorInternalServerError)?
    {
        return Ok(session_resp("").json(body_failure("Incorrect password.")));
    };

    let session = UserSession::create(conn, user.uuid)
        .await
        .map_err(ErrorInternalServerError)?;

    Ok(session_resp(&session.key_text()).json(body_success("Logged in.")))
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

async fn logout(pool: web::Data<SqlitePool>, session: SessionOpt) -> Resp {
    if let SessionOpt::Some(session) = session {
        let conn = &mut pool.acquire().await.map_err(ErrorInternalServerError)?;
        session.session.end(conn).await.ok();
    }

    Ok(session_resp("").json(body_success("Logged out.")))
}

#[cfg(test)]
mod test {
    use actix_web::{cookie::Cookie, test, web::Data, App};

    use super::LoginRequest;
    use crate::{
        api::{routes, Binary},
        fs::initialise_database,
        models::User,
        req::session::COOKIE_NAME,
    };

    #[actix_web::test]
    async fn test_auth_api() {
        // Test POST /api/auth/login, POST /api/auth/test, POST /api/auth/logout

        let db = initialise_database().await.unwrap();
        let app =
            test::init_service(App::new().app_data(Data::new(db.clone())).service(routes())).await;

        let conn = &mut db.acquire().await.unwrap();
        let user = User::generate(conn).await;

        // Test with no session should fail.
        let req = test::TestRequest::post().uri("/api/auth/test").to_request();
        let resp: Binary = test::call_and_read_body_json(&app, req).await;
        assert!(!resp.success);

        // Log in with bad password should fail.
        let req = test::TestRequest::post()
            .uri("/api/auth/login")
            .set_json(LoginRequest {
                username: user.username.clone(),
                password: "wrongpassword".into(),
            })
            .to_request();
        let resp: Binary = test::call_and_read_body_json(&app, req).await;
        assert!(!resp.success);

        // Log in with correct password should succeed, setting up session.
        let req = test::TestRequest::post()
            .uri("/api/auth/login")
            .set_json(LoginRequest {
                username: user.username.clone(),
                password: "password".into(),
            })
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let cookie =
            Cookie::parse(resp.headers().get("Set-Cookie").unwrap().to_str().unwrap()).unwrap();
        assert_eq!(cookie.name(), COOKIE_NAME);

        // Test with no session should still fail.
        let req = test::TestRequest::post().uri("/api/auth/test").to_request();
        let resp: Binary = test::call_and_read_body_json(&app, req).await;
        assert!(!resp.success);

        // Test with session cookie should succeed.
        let req = test::TestRequest::post()
            .uri("/api/auth/test")
            .cookie(cookie.clone())
            .to_request();
        let resp: Binary = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);

        // Log out with session should succeed.
        let req = test::TestRequest::post()
            .uri("/api/auth/logout")
            .cookie(cookie.clone())
            .to_request();
        let resp: Binary = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);

        // Test with expired session should succeed.
        let req = test::TestRequest::post()
            .uri("/api/auth/test")
            .cookie(cookie)
            .to_request();
        let resp: Binary = test::call_and_read_body_json(&app, req).await;
        assert!(!resp.success);
    }
}
