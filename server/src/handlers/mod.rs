use sqlx::SqlitePool;
use warp::Filter;

mod common;
mod register;

type ConfiguredFilter = impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone;

pub fn filters(pool: SqlitePool) -> ConfiguredFilter {
    register::filter(pool)
}

