use actix_web::{error::ErrorInternalServerError, web, HttpResponse};
use sqlx::SqliteConnection;

use super::{res_failure, res_json, res_success};
use crate::models::{Project, SceneRecord, User};
use crate::req::Conn;
use crate::utils::e500;

pub fn routes() -> actix_web::Scope {
    web::scope("/project")
        .route("/list", web::get().to(list))
        .route("/new", web::post().to(new))
        .route("/{project_id}", web::get().to(get))
        .route("/{project_id}", web::delete().to(delete))
}

#[derive(serde_derive::Serialize)]
struct SceneListEntry {
    scene_key: String,
    title: String,
    updated_time: i64,
    thumbnail: String,
}

impl SceneListEntry {
    fn from(scene: SceneRecord) -> Self {
        SceneListEntry {
            scene_key: scene.scene_key,
            title: scene.title,
            updated_time: scene.updated_time,
            thumbnail: scene.thumbnail,
        }
    }
}

#[derive(serde_derive::Serialize)]
struct ProjectListEntry {
    id: i64,
    project_key: String,
    title: String,
    scene_list: Vec<SceneListEntry>,
}

impl ProjectListEntry {
    async fn from(project: Project, conn: &mut SqliteConnection) -> anyhow::Result<Self> {
        let scene_list = project
            .list_scenes(conn)
            .await?
            .into_iter()
            .map(SceneListEntry::from)
            .collect();
        Ok(ProjectListEntry {
            id: project.id,
            project_key: project.project_key,
            title: project.title,
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
    let mut projects = Project::list(conn.acquire(), user.id)
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
    project_key: String,
    title: String,
}

async fn new(
    mut conn: Conn,
    user: User,
    req: web::Json<NewProjectRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    let project = Project::new(conn.acquire(), user.id, &req.title)
        .await
        .map_err(e500)?;

    res_json(NewProjectResponse {
        message: "Project created successfully.".to_owned(),
        success: true,
        project_key: project.project_key,
        title: req.title.clone(),
    })
}

async fn get(
    mut conn: Conn,
    user: User,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, actix_web::Error> {
    let project_key = path.into_inner().0;
    let project = Project::get_by_key(conn.acquire(), &project_key)
        .await
        .map_err(e500)?;

    if project.user != user.id {
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
    let project_key = path.into_inner().0;
    let project = Project::get_by_key(conn.acquire(), &project_key)
        .await
        .map_err(e500)?;

    if project.user != user.id {
        res_failure("Project not found.")
    } else {
        project.delete(conn.acquire()).await.map_err(e500)?;
        res_success("Project deleted successfully.")
    }
}
