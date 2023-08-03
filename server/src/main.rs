#![feature(const_trait_impl)]
#![allow(dead_code)]
#![allow(opaque_hidden_inferred_bound)]
#![allow(clippy::too_many_arguments)]

/// opaque_hidden_inferred_bound is needed because there is an implied bound of
/// `warp::generic::Tuple`, which is private.
use std::{collections::HashMap, path::PathBuf};

use actix_files::NamedFile;
use actix_web::{middleware::Logger, web, App, HttpServer};
use games::GameHandle;
use once_cell::sync::Lazy;
pub use scene;
use sqlx::sqlite::SqlitePool;
use tokio::sync::RwLock;

mod api;
mod crypto;
mod games;
mod models;
mod utils;

const USAGE: &str = "Usage: ./server content/ 80";
pub static CONTENT: Lazy<PathBuf> =
    Lazy::new(|| PathBuf::from(std::env::args().nth(1).expect(USAGE)));

async fn content(path: &str) -> std::io::Result<NamedFile> {
    NamedFile::open_async(CONTENT.join(path)).await
}

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
    api::set_pool(pool.clone());

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
            .service(web::resource("/login").to(|| content("login.html")))
            .service(web::resource("/register").to(|| content("register.html")))
            .service(web::resource("/scene").to(|| content("scene.html")))
            .service(web::resource("/media").to(|| content("media.html")))
            .service(web::resource("/game_over").to(|| content("game_over.html")))
            .service(web::resource("/landing").to(|| content("landing.html")))
            .service(
                web::scope("/project")
                    .service(web::resource("/new").to(|| content("new_project.html")))
                    .service(web::resource("/{proj_key}").to(|| content("edit_project.html")))
                    .service(
                        web::resource("/{proj_key}/scene/{scene_key}").to(|| content("scene.html")),
                    )
                    .default_service(web::route().to(|| content("projects.html"))),
            )
            .service(
                web::scope("/game")
                    .service(web::resource("/{game_key}").to(|| content("scene.html")))
                    .default_service(web::route().to(|| content("game.html"))),
            )
            .service(actix_files::Files::new("/", CONTENT.clone()).index_file("index.html"))
    })
    .bind((std::net::Ipv4Addr::new(0, 0, 0, 0), port))?
    .run()
    .await
}
