use actix_web::{error::ErrorInternalServerError, web, HttpResponse};
use sqlx::SqliteConnection;
use uuid::Uuid;

use super::{res_failure, res_json, res_success};
use crate::models::{Project, Scene, User};
use crate::req::{e500, Conn};
use crate::utils::{format_uuid, Res};

pub fn routes() -> actix_web::Scope {
    web::scope("/project")
        .route("/list", web::get().to(list))
        .route("/new", web::post().to(new))
        .route("/{project_id}", web::get().to(get))
        .route("/{project_id}", web::delete().to(delete))
}

#[derive(serde_derive::Serialize)]
struct SceneListEntry {
    uuid: String,
    title: String,
    updated_time: i64,
    thumbnail: Option<String>,
}

impl SceneListEntry {
    fn from(scene: Scene) -> Self {
        SceneListEntry {
            uuid: format_uuid(scene.uuid),
            title: scene.title.unwrap_or_else(|| "Untitled".to_string()),
            updated_time: scene.updated_time,
            thumbnail: scene.thumbnail,
        }
    }
}

#[derive(serde_derive::Serialize)]
struct ProjectListEntry {
    uuid: String,
    title: String,
    scene_list: Vec<SceneListEntry>,
}

impl ProjectListEntry {
    async fn from(project: Project, conn: &mut SqliteConnection) -> Res<Self> {
        let scene_list = project
            .list_scenes(conn)
            .await?
            .into_iter()
            .map(SceneListEntry::from)
            .collect();
        Ok(ProjectListEntry {
            uuid: format_uuid(project.uuid),
            title: project.title.unwrap_or_else(|| "Untitled".to_string()),
            scene_list,
        })
    }
}

#[derive(serde_derive::Serialize)]
struct ProjectResponse {
    message: String,
    success: bool,
    project: ProjectListEntry,
}

#[derive(serde_derive::Serialize)]
struct ProjectListResponse {
    message: String,
    success: bool,
    list: Vec<ProjectListEntry>,
}

async fn list(mut conn: Conn, user: User) -> Result<HttpResponse, actix_web::Error> {
    let mut projects = Project::list_for_user(conn.acquire(), user.uuid)
        .await
        .map_err(ErrorInternalServerError)?;

    let mut project_list = vec![];
    while let Some(project) = projects.pop() {
        let entry = ProjectListEntry::from(project, conn.acquire())
            .await
            .map_err(e500)?;
        project_list.push(entry);
    }

    Ok(HttpResponse::Ok().json(&ProjectListResponse {
        message: "Project list retrieved.".to_string(),
        success: true,
        list: project_list,
    }))
}

#[derive(serde_derive::Deserialize)]
struct NewProjectRequest {
    title: String,
}

#[derive(serde_derive::Serialize)]
struct NewProjectResponse {
    message: String,
    success: bool,
    uuid: String,
    title: String,
}

async fn new(
    mut conn: Conn,
    user: User,
    req: web::Json<NewProjectRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let project = Project::create(conn.acquire(), user.uuid, &req.title)
        .await
        .map_err(e500)?;

    res_json(NewProjectResponse {
        message: "Project created successfully.".to_owned(),
        success: true,
        uuid: format_uuid(project.uuid),
        title: req.title.clone(),
    })
}

fn retrieve_uuid_from_path(path: web::Path<(String,)>) -> Result<Uuid, actix_web::Error> {
    Uuid::try_parse(&path.into_inner().0)
        .map_err(|e| actix_web::error::ErrorUnprocessableEntity(format!("Invalid UUID: {e}")))
}

async fn get(
    mut conn: Conn,
    user: User,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, actix_web::Error> {
    let project = retrieve_uuid_from_path(path)?;
    let project = Project::load_by_uuid(conn.acquire(), project)
        .await
        .map_err(e500)?;

    if project.user != user.uuid {
        res_failure("Project not found.")
    } else {
        let list = ProjectListEntry::from(project, conn.acquire())
            .await
            .map_err(e500)?;
        res_json(list)
    }
}

async fn delete(
    mut conn: Conn,
    user: User,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, actix_web::Error> {
    let project = retrieve_uuid_from_path(path)?;
    let project = Project::load_by_uuid(conn.acquire(), project)
        .await
        .map_err(e500)?;

    if project.user != user.uuid {
        res_failure("Project not found.")
    } else {
        project.delete(conn.acquire()).await.map_err(e500)?;
        res_success("Project deleted successfully.")
    }
}
