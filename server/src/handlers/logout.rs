use std::convert::Infallible;

use sqlx::SqlitePool;
use warp::Filter;

use super::response::Binary;
use super::{current_time, with_db, with_session};

async fn end_session(pool: &SqlitePool, session_key: &str) -> anyhow::Result<bool> {
    let rows_affected =
        sqlx::query("UPDATE user_sessions SET end_time = ?1 WHERE session_key = ?2")
            .bind(current_time()? as i64)
            .bind(session_key)
            .execute(pool)
            .await?
            .rows_affected();

    Ok(rows_affected > 0)
}

async fn logout(pool: SqlitePool, session_key: String) -> Result<impl warp::Reply, Infallible> {
    end_session(&pool, session_key.as_str()).await.ok();
    Binary::result_success("Logged out.")
}

pub fn filter(
    pool: SqlitePool,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("logout")
        .and(warp::post())
        .and(with_db(pool))
        .and(with_session())
        .and_then(logout)
}
