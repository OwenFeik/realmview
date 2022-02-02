#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::sqlite::SqlitePool;
use tokio::sync::RwLock;
use warp::Filter;

mod crypto;
mod game;
mod handlers;
mod models;

use game::Games;

async fn connect_to_db() -> SqlitePool {
    SqlitePool::connect(
        std::env::var("DATABASE_URL")
            .expect("DATABASE_URL not set")
            .as_str(),
    )
    .await
    .expect("Database pool creation failed.")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let sessions: Games = Arc::new(RwLock::new(HashMap::new()));

    let content_dir = std::env::args().nth(1).expect("Usage: ./server content/");
    let pool = connect_to_db().await;
    let route = warp::fs::dir(content_dir.clone()).or(handlers::routes(pool, content_dir));

    warp::serve(route).run(([127, 0, 0, 1], 3030)).await;

    Ok(())
}
