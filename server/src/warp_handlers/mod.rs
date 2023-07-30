use std::{
    convert::Infallible,
    io::Read,
    path::{Path, PathBuf},
};

use bincode::deserialize;
use sqlx::SqlitePool;
use warp::Filter;

use crate::models::User;

mod auth;
mod game;
mod media;
mod project;
mod register;
mod scene;
mod upload;

pub fn routes(
    pool: SqlitePool,
    games: crate::games::Games,
    content_dir: String,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let content_path = PathBuf::from(content_dir.clone());
    warp::path("api")
        .and(
            auth::filter(pool.clone())
                .or(register::filter(pool.clone()))
                .or(upload::filter(pool.clone(), content_dir.clone()))
                .or(media::filter(pool.clone(), content_dir))
                .or(project::filter(pool.clone()))
                .or(game::routes(pool.clone(), games))
                .or(scene::routes(pool)),
        )
        .or(warp::fs::dir(content_path.clone()))
        .or(page_routes(&content_path))
}

fn page_routes(
    dir: &Path,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let serve_scene = warp::fs::file(dir.join("scene.html"));
    let serve_game = warp::fs::file(dir.join("game.html"));

    warp::get().and(
        (warp::path::end().and(warp::fs::file(dir.join("index.html"))))
            .or(warp::path!("login").and(warp::fs::file(dir.join("login.html"))))
            .or(warp::path!("register").and(warp::fs::file(dir.join("register.html"))))
            .or(warp::path!("scene").and(serve_scene.clone()))
            .or(warp::path!("media").and(warp::fs::file(dir.join("media.html"))))
            .or(warp::path!("project" / "new")
                .and(warp::path::end())
                .and(warp::fs::file(dir.join("new_project.html"))))
            .or(warp::path!("project" / String)
                .and(warp::path::end())
                .map(|_| ())
                .untuple_one()
                .and(warp::fs::file(dir.join("edit_project.html"))))
            .or(warp::path!("project" / String / "scene" / String)
                .map(|_proj_key, _scene_key| {}) // TODO could validate
                .untuple_one()
                .and(serve_scene.clone()))
            .or(warp::path!("project").and(warp::fs::file(dir.join("projects.html"))))
            .or(warp::path!("game" / String / "client" / String)
                .map(|_game_key, _client_key| {})
                .untuple_one()
                .and(serve_scene))
            .or(warp::path!("game" / String)
                .map(|_game_key| {})
                .untuple_one()
                .and(serve_game.clone()))
            .or(warp::path("game").and(serve_game))
            .or(warp::path("game_over").and(warp::fs::file(dir.join("game_over.html"))))
            .or(warp::path("landing").and(warp::fs::file(dir.join("landing.html")))),
    )
}

pub fn json_body<T: std::marker::Send + for<'de> serde::Deserialize<'de>>(
) -> impl Filter<Extract = (T,), Error = warp::Rejection> + Clone {
    warp::body::content_length_limit(1024 * 16).and(warp::body::json())
}

pub fn binary_body<T: std::marker::Send + for<'de> serde::Deserialize<'de>>(
) -> impl Filter<Extract = (T,), Error = warp::Rejection> + Clone {
    warp::body::content_length_limit(1024 * 16)
        .and(warp::body::bytes())
        .and_then(|bytes: bytes::Bytes| async move {
            let data: Result<Vec<u8>, _> = bytes.bytes().collect();
            if let Ok(b) = data {
                if let Ok(t) = deserialize(&b) {
                    return Ok(t);
                }
            }
            Err(warp::reject::reject())
        })
}

pub fn with_db(
    pool: SqlitePool,
) -> impl Filter<Extract = (SqlitePool,), Error = Infallible> + Clone {
    warp::any().map(move || pool.clone())
}

pub fn parse_cookie(cookies: String, goal_key: &str) -> Option<String> {
    for cookie in cookies.split(';') {
        let parts = cookie.splitn(2, '=').collect::<Vec<&str>>();
        if let Some(key) = parts.first() {
            if key.trim() == goal_key {
                return parts.get(1).map(|s| String::from(s.trim()));
            }
        }
    }

    None
}

async fn session_key(cookies: String) -> Result<String, warp::Rejection> {
    match parse_cookie(cookies, "session_key") {
        Some(skey) => Ok(skey),
        None => Err(warp::reject()),
    }
}

pub fn with_session() -> impl Filter<Extract = (String,), Error = warp::Rejection> + Clone {
    warp::header("Cookie").and_then(session_key)
}

pub async fn user_from_cookie(
    (pool, cookie): (SqlitePool, String),
) -> Result<User, warp::Rejection> {
    match parse_cookie(cookie, "session_key") {
        Some(skey) => match User::get_by_session(&pool, &skey).await {
            Ok(Some(user)) => Ok(user),
            _ => Err(warp::reject()),
        },
        None => Err(warp::reject()),
    }
}

pub async fn with_user(
    pool: SqlitePool,
) -> impl Filter<Extract = (User,), Error = warp::Rejection> + Clone {
    warp::header::header("Cookie")
        .map(move |s| (pool.clone(), s))
        .and_then(user_from_cookie)
}

pub fn with_val<T: Clone + std::marker::Send>(
    val: T,
) -> impl Filter<Extract = (T,), Error = Infallible> + Clone {
    warp::any().map(move || val.clone())
}

pub mod response {
    use std::convert::Infallible;

    use serde::Serialize;
    use serde_derive::Serialize;
    use warp::http::StatusCode;

    type JsonReply = warp::reply::WithStatus<warp::reply::Json>;
    pub type ResultReply = Result<JsonReply, Infallible>;

    #[derive(Serialize)]
    pub struct Binary {
        message: String,
        success: bool,
    }

    impl Binary {
        pub fn new(message: &str, success: bool) -> Binary {
            Binary {
                message: String::from(message),
                success,
            }
        }

        pub fn new_success(message: &str) -> Binary {
            Binary::new(message, true)
        }

        pub fn new_failure(message: &str) -> Binary {
            Binary::new(message, false)
        }

        pub fn result_success(message: &str) -> ResultReply {
            as_result(&Binary::new_success(message), StatusCode::OK)
        }

        pub fn result_failure(message: &str) -> ResultReply {
            as_result(
                &Binary::new_failure(message),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
        }

        pub fn result_error(message: &str) -> ResultReply {
            as_result(
                &Binary::new_failure(message),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }

        pub fn from_error<E: std::fmt::Display>(err: E) -> ResultReply {
            Self::result_failure(&format!("Error: {err}"))
        }
    }

    pub fn as_reply(body: &impl Serialize, status: StatusCode) -> JsonReply {
        warp::reply::with_status(warp::reply::json(body), status)
    }

    pub fn as_result(body: &impl Serialize, status: StatusCode) -> ResultReply {
        Ok(as_reply(body, status))
    }

    pub fn cookie_result(
        body: &impl Serialize,
        status: StatusCode,
        key: &str,
        value: Option<&str>,
    ) -> Result<impl warp::Reply, Infallible> {
        let cookie = value.unwrap_or("");

        // SameSite=Strict causes the cookie to be sent only on requests from
        // this website to this website.
        //
        // Max-Age=15552000 causes the cookie to be retained for up to 6 months
        // unless cleared (manually or by logging out).
        let cookie = format!(
            "{}={}; SameSite=Strict; Max-Age=15552000; Path=/;",
            key, cookie
        );

        Ok(warp::reply::with_header(
            as_reply(&body, status),
            "Set-Cookie",
            cookie.as_str(),
        ))
    }

    fn invalid_session() -> Result<impl warp::Reply, Infallible> {
        cookie_result(
            &Binary::new_failure("Invalid session."),
            StatusCode::UNAUTHORIZED,
            "session_key",
            None,
        )
    }
}