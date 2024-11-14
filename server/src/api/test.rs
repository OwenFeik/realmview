use actix_web::{cookie::Cookie, test, web::Data, App};

use super::{routes, Binary};
use crate::{fs::initialise_database, models::User, req::session::COOKIE_NAME};

#[actix_web::test]
async fn test_auth() {
    // Test POST /auth/login, POST /auth/test, POST /auth/logout

    let db = initialise_database().await.unwrap();
    let app =
        test::init_service(App::new().app_data(Data::new(db.clone())).service(routes())).await;

    let conn = &mut db.acquire().await.unwrap();
    let user = User::generate(conn).await.unwrap();

    // Test with no session should fail.
    let req = test::TestRequest::post().uri("/api/auth/test").to_request();
    let resp: Binary = test::call_and_read_body_json(&app, req).await;
    assert!(!resp.success);

    // Log in with bad password should fail.
    let req = test::TestRequest::post()
        .uri("/api/auth/login")
        .append_header(("Content-Type", "application/json"))
        .set_payload(format!(
            r#"{{"username":"{}","password":"{}"}}"#,
            &user.username, "wrongpassword"
        ))
        .to_request();
    let resp: Binary = test::call_and_read_body_json(&app, req).await;
    assert!(!resp.success);

    // Log in with correct password should succeed, setting up session.
    let req = test::TestRequest::post()
        .uri("/api/auth/login")
        .append_header(("Content-Type", "application/json"))
        .set_payload(format!(
            r#"{{"username":"{}","password":"{}"}}"#,
            &user.username, "password"
        ))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let cookie =
        Cookie::parse(resp.headers().get("Set-Cookie").unwrap().to_str().unwrap()).unwrap();
    assert_eq!(cookie.name(), COOKIE_NAME);
    let session = cookie.value();

    // Test with no session should still fail.
    let req = test::TestRequest::post().uri("/api/auth/test").to_request();
    let resp: Binary = test::call_and_read_body_json(&app, req).await;
    assert!(!resp.success);

    // Test with session cookie should succeed.
    let mut cookie = Cookie::named(COOKIE_NAME);
    cookie.set_value(session);
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
