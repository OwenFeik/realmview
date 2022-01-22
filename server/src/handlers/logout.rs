use std::convert::Infallible;

use sqlx::SqlitePool;
use warp::Filter;

use super::{current_time, with_db};
use super::response::Binary;


async fn end_session(pool: &SqlitePool, session_key: &str) -> anyhow::Result<bool> {
    let rows_affected = sqlx::query("UPDATE user_sessions SET end_time = ?1 WHERE session_key = ?2")
        .bind(current_time()? as i64)
        .bind(session_key)
        .execute(pool)
        .await?
        .rows_affected();

    Ok(rows_affected > 0)
}


fn parse_session_key(cookies: String) -> Option<String> {
    for cookie in cookies.split(";") {
        let parts: Vec<&str> = cookie.splitn(2, "=").collect::<Vec<&str>>();
        if let Some(key) = parts.get(0) {
            if key.trim() == "session_key" {
                return parts.get(1).map(|s| String::from(s.trim()));
            }
        }
    }

    None
}


async fn logout(pool: SqlitePool, cookies: Option<String>) -> Result<impl warp::Reply, Infallible> {
    if let Some(cookies) = cookies {
        if let Some(skey) = parse_session_key(cookies) {
            end_session(&pool, skey.as_str()).await.ok();
        }
    }
    

    Binary::result_success("Logged out.")
}


pub fn filter(pool: SqlitePool) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("logout")
        .and(warp::post())
        .and(with_db(pool))
        .and(warp::filters::header::optional("Cookie"))
        .and_then(logout)
}
