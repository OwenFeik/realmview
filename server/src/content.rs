use actix_files::NamedFile;
use actix_web::{web, FromRequest, HttpRequest, HttpResponse};
use uuid::Uuid;

use crate::{
    fs::CONTENT,
    models::{Project, User},
    req::{
        e500,
        session::{Session, SessionOpt},
        Pool,
    },
};

pub fn routes() -> actix_web::Scope {
    web::scope("")
        .route("/login", public(files::LOGIN))
        .route("/register", public(files::REGISTER))
        .route("/media", loggedin(files::MEDIA))
        .route("/game_over", public(files::GAME_OVER))
        .route("/disconnected", public(files::DISCONNECTED))
        .route("/landing", loggedin(files::LANDING))
        .route("/not_found", public(files::NOT_FOUND))
        .service(projects())
        .service(game())
        .route("/", public(files::INDEX))
        .service(actix_files::Files::new("/", &*CONTENT).index_file(files::INDEX))
}

fn projects() -> actix_web::Scope {
    web::scope("/project")
        .route("/new", loggedin(files::NEW_PROJECT))
        .route("/{uuid}", loggedin(files::EDIT_PROJECT))
        .route("/{uuid}/edit", loggedin(files::EDITOR))
        .default_service(loggedin(files::PROJECTS))
}

fn game() -> actix_web::Scope {
    web::scope("game")
        .route("/{game_key}", loggedin(files::EDITOR))
        .default_service(loggedin(files::GAME))
}

fn public(path: &'static str) -> actix_web::Route {
    web::get().to(|| content(path))
}

fn loggedin(path: &'static str) -> actix_web::Route {
    web::get().to(move |req| loggedin_content(req, path))
}

async fn loggedin_content(req: HttpRequest, path: &str) -> Result<NamedFile, actix_web::Error> {
    Session::from_request(&req, &mut actix_web::dev::Payload::None).await?;
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

async fn new_scene(
    req: HttpRequest,
    mut conn: Pool,
    user: User,
    path: web::Path<(Uuid,)>,
) -> Result<HttpResponse, actix_web::Error> {
    let project_key = path.into_inner().0;
    let proj = Project::get_by_uuid(conn.acquire(), project_key)
        .await
        .map_err(e500)?;

    if proj.user != user.uuid {
        return Ok(content(files::NEW_PROJECT)
            .await
            .map_err(e500)?
            .into_response(&req));
    }

    todo!("refactor to project-based scene editor");

    // let scene = scene::Scene::new();
    // let scene_key = proj
    //     .update_scene(conn.acquire(), scene)
    //     .await
    //     .map_err(e500)?
    //     .scene_key;

    // Ok(redirect(&format!(
    //     "/project/{project_key}/scene/{scene_key}"
    // )))
}

mod files {
    pub const EDIT_PROJECT: &str = "edit_project.html";
    pub const GAME: &str = "game.html";
    pub const GAME_OVER: &str = "game_over.html";
    pub const DISCONNECTED: &str = "disconnected.html";
    pub const INDEX: &str = "index.html";
    pub const LANDING: &str = "landing.html";
    pub const LOGIN: &str = "login.html";
    pub const MEDIA: &str = "media.html";
    pub const NEW_PROJECT: &str = "new_project.html";
    pub const NOT_FOUND: &str = "not_found.html";
    pub const PROJECTS: &str = "projects.html";
    pub const REGISTER: &str = "register.html";
    pub const EDITOR: &str = "editor.html";
}
