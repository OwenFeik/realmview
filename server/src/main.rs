#![feature(let_chains)]
#![feature(thread_id_value)]
#![allow(dead_code)]

use std::collections::HashMap;

use actix_web::{middleware::Logger, web::Data, App, HttpServer};
use games::{GameHandle, GameKey};

mod api;
mod content;
mod crypto;
mod fs;
mod games;
mod models;
mod req;
mod utils;

pub use scene;
use tokio::sync::RwLock;

const USAGE: &str = "Usage: DATA_DIR=. ./server 80";

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let port = std::env::args()
        .nth(1)
        .expect(USAGE)
        .parse::<u16>()
        .expect("Invalid port number.");

    let db = fs::initialise_database()
        .await
        .expect("Database initialisation failed.");

    let games: Data<RwLock<HashMap<GameKey, GameHandle>>> =
        Data::new(RwLock::new(HashMap::<GameKey, GameHandle>::new()));

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
            .app_data(Data::new(db.clone()))
            .app_data(Data::clone(&games))
            .service(api::routes())
            .service(content::routes())
    })
    .bind((std::net::Ipv4Addr::new(0, 0, 0, 0), port))?
    .run()
    .await
}
