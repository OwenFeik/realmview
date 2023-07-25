#![allow(dead_code)]
#![allow(opaque_hidden_inferred_bound)]
#![allow(clippy::too_many_arguments)]

/// opaque_hidden_inferred_bound is needed because there is an implied bound of
/// `warp::generic::Tuple`, which is private.
use std::collections::HashMap;
use std::sync::Arc;

pub use scene;
use sqlx::sqlite::SqlitePool;
use tokio::sync::RwLock;

mod crypto;
mod games;
mod handlers;
mod models;
mod utils;

use games::Games;

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
    const USAGE: &str = "Usage: ./server content/ 80";

    let games: Games = Arc::new(RwLock::new(HashMap::new()));
    let pool = connect_to_db().await;
    let content_dir = std::env::args().nth(1).expect(USAGE);
    let route = handlers::routes(pool, games, content_dir);
    let port = std::env::args()
        .nth(2)
        .expect(USAGE)
        .parse::<u16>()
        .expect("Invalid port number.");
    warp::serve(route).run(([0, 0, 0, 0], port)).await;

    Ok(())
}
