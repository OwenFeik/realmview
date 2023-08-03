use std::path::PathBuf;

use actix_files::NamedFile;
use actix_web::web;
use once_cell::sync::Lazy;

use crate::req::session::SessionOpt;

pub static CONTENT: Lazy<PathBuf> =
    Lazy::new(|| PathBuf::from(std::env::args().nth(1).expect(super::USAGE)));

pub fn routes() -> actix_web::Scope {
    web::scope("")
        .service(web::resource("/login").to(|| content(files::LOGIN)))
        .service(web::resource("/register").to(|| content(files::REGISTER)))
        .service(web::resource("/scene").to(|| content(files::SCENE)))
        .service(web::resource("/media").to(|| content(files::MEDIA)))
        .service(web::resource("/game_over").to(|| content(files::GAME_OVER)))
        .service(web::resource("/landing").to(|| content(files::LANDING)))
        .service(
            web::scope("/project")
                .service(web::resource("/new").to(|| content(files::NEW_PROJECT)))
                .service(web::resource("/{proj_key}").to(|| content(files::EDIT_PROJECT)))
                .service(
                    web::resource("/{proj_key}/scene/{scene_key}").to(|| content(files::SCENE)),
                )
                .default_service(web::route().to(|| content(files::PROJECTS))),
        )
        .service(
            web::scope("/game")
                .service(web::resource("/{game_key}").to(|| content(files::SCENE)))
                .default_service(web::route().to(|| content(files::GAME))),
        )
        .route("/", web::get().to(landing))
        .service(actix_files::Files::new("/", CONTENT.clone()).index_file(files::INDEX))
}

async fn content(path: &str) -> std::io::Result<NamedFile> {
    NamedFile::open_async(CONTENT.join(path)).await
}

async fn landing(sess: SessionOpt) -> std::io::Result<NamedFile> {
    match sess {
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
