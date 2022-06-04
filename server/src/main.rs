#![allow(dead_code)]
#![feature(type_alias_impl_trait)]

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::sqlite::SqlitePool;
use tokio::sync::RwLock;

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
    let games: Games = Arc::new(RwLock::new(HashMap::new()));
    let pool = connect_to_db().await;
    let content_dir = std::env::args().nth(1).expect("Usage: ./server content/");
    let route = handlers::routes(pool, games, content_dir);

    warp::serve(route).run(([127, 0, 0, 1], 3030)).await;

    Ok(())
}
