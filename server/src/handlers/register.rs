use std::convert::Infallible;

use serde_derive::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Sqlite;
use warp::{Filter, http::StatusCode};


use crate::handlers::common::*;

#[derive(Deserialize, Serialize)]
struct Registration {
    username: String,
    password: String
}


async fn username_taken(details: Registration, pool: SqlitePool) -> anyhow::Result<bool> {
    let row = sqlx::query::<Sqlite>("SELECT id FROM users WHERE username = ?1;")
        .bind::<&str>(details.username.as_str())
        .fetch_optional(&pool)
        .await?;

    Ok(row.is_none())
}

async fn register(details: Registration, pool: SqlitePool) -> Result<impl warp::Reply, Infallible> {
    match username_taken(details, pool).await {
        Ok(true) => return Ok(StatusCode::OK),
        Ok(false) => (),
        Err(_) => return Ok(StatusCode::INTERNAL_SERVER_ERROR) 
    };


    
    
    Ok(StatusCode::ACCEPTED)
}

pub fn filter(pool: SqlitePool) -> crate::handlers::ConfiguredFilter {
    warp::path!("register")
        .and(warp::post())
        .and(json_body::<Registration>())
        .and(with_db(pool))
        .and_then(register)
}
