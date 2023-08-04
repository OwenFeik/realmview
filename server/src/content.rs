use std::path::PathBuf;

use actix_files::NamedFile;
use actix_web::{web, FromRequest, HttpRequest};
use once_cell::sync::Lazy;

use crate::req::{
    e500,
    session::{Session, SessionOpt},
};

pub static CONTENT: Lazy<PathBuf> =
    Lazy::new(|| PathBuf::from(std::env::args().nth(1).expect(super::USAGE)));

pub fn routes() -> actix_web::Scope {
    web::scope("")
        .route("/login", public(files::LOGIN))
        .route("/register", public(files::REGISTER))
        .route("/media", loggedin(files::MEDIA))
        .route("/game_over", public(files::GAME_OVER))
        .route("/landing", loggedin(files::LANDING))
        .service(projects())
        .service(game())
        .route("/", public(files::INDEX))
        .service(actix_files::Files::new("/", &*CONTENT).index_file(files::INDEX))
}

fn projects() -> actix_web::Scope {
    web::scope("/project")
        .route("/new", loggedin(files::NEW_PROJECT))
        .route("/{proj_key}", loggedin(files::EDIT_PROJECT))
        .route("/{proj_key}/scene/{scene_key}", loggedin(files::SCENE))
        .default_service(loggedin(files::PROJECTS))
}

fn game() -> actix_web::Scope {
    web::scope("game")
        .route("/{game_key}", loggedin(files::SCENE))
        .default_service(loggedin(files::GAME))
}

fn public(path: &'static str) -> actix_web::Route {
    web::get().to(|| content(path))
}

fn loggedin(path: &'static str) -> actix_web::Route {
    web::get().to(async move |req| loggedin_content(&req, path).await)
}

async fn loggedin_content(req: &HttpRequest, path: &str) -> Result<NamedFile, actix_web::Error> {
    Session::from_request(req, &mut actix_web::dev::Payload::None).await?;
    content(path).await.map_err(e500)
}

async fn content(path: &str) -> std::io::Result<NamedFile> {
    NamedFile::open_async(CONTENT.join(path)).await
}

async fn index(session: SessionOpt) -> std::io::Result<NamedFile> {
    match session {
        SessionOpt::Some(_) => content(files::LANDING).await,
        SessionOpt::None => content(files::INDEX).await,
    }
}

mod files {
    pub const EDIT_PROJECT: &str = "edit_project.html";
    pub const GAME: &str = "game.html";
    pub const GAME_OVER: &str = "game_over.html";
    pub const INDEX: &str = "index.html";
    pub const LANDING: &str = "landing.html";
    pub const LOGIN: &str = "login.html";
    pub const MEDIA: &str = "media.html";
    pub const NEW_PROJECT: &str = "new_project.html";
    pub const PROJECTS: &str = "projects.html";
    pub const REGISTER: &str = "register.html";
    pub const SCENE: &str = "scene.html";
}
