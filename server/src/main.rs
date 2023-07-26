#![allow(dead_code)]
#![allow(opaque_hidden_inferred_bound)]
#![allow(clippy::too_many_arguments)]

/// opaque_hidden_inferred_bound is needed because there is an implied bound of
/// `warp::generic::Tuple`, which is private.
use std::{collections::HashMap, path::PathBuf};

use actix_web::{web, App, HttpServer};
pub use scene;
use sqlx::sqlite::SqlitePool;
use tokio::sync::RwLock;

mod auth;
mod crypto;
mod games;
mod handlers;
mod models;
mod utils;

use games::{GameRef, Games};

async fn connect_to_db() -> SqlitePool {
    SqlitePool::connect(
        std::env::var("DATABASE_URL")
            .expect("DATABASE_URL not set")
            .as_str(),
    )
    .await
    .expect("Database pool creation failed.")
}

struct Config {
    pub content_dir: PathBuf,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    const USAGE: &str = "Usage: ./server content/ 80";

    let port = std::env::args()
        .nth(2)
        .expect(USAGE)
        .parse::<u16>()
        .expect("Invalid port number.");

    HttpServer::new(|| {
        let content_dir = PathBuf::from(std::env::args().nth(1).expect(USAGE));
        App::new()
            .app_data(web::Data::new(connect_to_db()))
            .app_data(web::Data::new(Config {
                content_dir: content_dir.clone(),
            }))
            .app_data(web::Data::new(RwLock::new(
                HashMap::<String, GameRef>::new(),
            )))
            .service(actix_files::Files::new("/", content_dir))
    })
    .bind((std::net::Ipv4Addr::new(0, 0, 0, 0), port))?
    .run()
    .await
}
