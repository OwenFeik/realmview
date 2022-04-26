use std::{convert::Infallible, io::Read};

use bincode::deserialize;
use sqlx::SqlitePool;
use warp::Filter;

use crate::models::User;

mod game;
mod login;
mod logout;
mod media;
mod project;
mod register;
mod scene;
mod upload;

pub fn routes(
    pool: SqlitePool,
    games: crate::game::Games,
    content_dir: String,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    login::filter(pool.clone())
        .or(register::filter(pool.clone()))
        .or(logout::filter(pool.clone()))
        .or(upload::filter(pool.clone(), content_dir))
        .or(media::filter(pool.clone()))
        .or(game::routes(pool.clone(), games))
        .or(scene::routes(pool))
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

pub fn current_time() -> anyhow::Result<u64> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs())
}

fn parse_cookie(cookies: String, goal_key: &str) -> Option<String> {
    for cookie in cookies.split(';') {
        let parts = cookie.splitn(2, '=').collect::<Vec<&str>>();
        if let Some(key) = parts.get(0) {
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
            as_result(&Binary::new_failure(message), StatusCode::OK)
        }

        pub fn result_error(message: &str) -> ResultReply {
            as_result(
                &Binary::new_failure(message),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
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
        let cookie = format!("{}={}; SameSite=Strict; Max-Age=15552000;", key, cookie);

        Ok(warp::reply::with_header(
            as_reply(&body, status),
            "Set-Cookie",
            cookie.as_str(),
        ))
    }
}
