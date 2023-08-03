#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]

use std::collections::HashMap;

use actix_web::{middleware::Logger, web, App, HttpServer};
use games::GameHandle;
use sqlx::sqlite::SqlitePool;
use tokio::sync::RwLock;

mod api;
mod content;
mod crypto;
mod games;
mod models;
mod req;
mod utils;

pub use content::CONTENT;
pub use scene;

const USAGE: &str = "Usage: ./server content/ 80";

async fn connect_to_db() -> SqlitePool {
    SqlitePool::connect(
        std::env::var("DATABASE_URL")
            .expect("DATABASE_URL not set")
            .as_str(),
    )
    .await
    .expect("Database pool creation failed.")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let port = std::env::args()
        .nth(2)
        .expect(USAGE)
        .parse::<u16>()
        .expect("Invalid port number.");

    let pool = connect_to_db().await;
    req::set_pool(pool.clone());

    let games = web::Data::new(RwLock::new(HashMap::<String, GameHandle>::new()));

    // Every interval, drop all game servers which are no longer running.
    const GAMES_CLEANUP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);
    let games_ref = games.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(GAMES_CLEANUP_INTERVAL);
        loop {
            interval.tick().await;
            games_ref.write().await.retain(|_, server| server.open());
        }
    });

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::clone(&games))
            .service(api::routes())
            .service(content::routes())
    })
    .bind((std::net::Ipv4Addr::new(0, 0, 0, 0), port))?
    .run()
    .await
}
