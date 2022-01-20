use std::convert::Infallible;

use sqlx::sqlite::SqlitePool;
use warp::Filter;

pub fn json_body<T: std::marker::Send + for<'de> serde::Deserialize<'de>>() ->
    impl Filter<Extract = (T,), Error = warp::Rejection> + Clone
{
    warp::body::content_length_limit(1024 * 16).and(warp::body::json())
}

pub fn with_db(pool: SqlitePool) -> impl Filter<Extract = (SqlitePool,), Error = Infallible> + Clone {
    warp::any().map(move || pool.clone())
}

