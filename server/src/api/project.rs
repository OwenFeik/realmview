use actix_web::{error::ErrorInternalServerError, web, HttpResponse};
use sqlx::SqliteConnection;
use uuid::Uuid;

use super::{res_failure, res_json, res_success, res_unproc};
use crate::models::{Project, Scene, User};
use crate::req::{e500, Pool};
use crate::utils::{format_uuid, Res};

pub fn routes() -> actix_web::Scope {
    web::scope("/project")
        .route("/save", web::post().to(save))
        .route("/list", web::get().to(list))
        .route("/new", web::post().to(new))
        .route("/{project_id}", web::get().to(get))
        .route("/{project_id}", web::delete().to(delete))
}

#[derive(serde_derive::Serialize)]
struct ProjectResponse {
    message: String,
    success: bool,
    project: ProjectListEntry,
}

async fn save(
    mut pool: Pool,
    user: User,
    body: bytes::Bytes,
) -> Result<HttpResponse, actix_web::Error> {
    let conn = pool.acquire();

    let Ok(project) = scene::serde::deserialise(&body) else {
        return res_unproc("Failed to decode project.");
    };

    let (record, scenes) = match Project::save(conn, user.uuid, project).await {
        Ok(record) => record,
        Err(e) => return Err(e500(e)),
    };

    let scene_list = scenes.into_iter().map(SceneListEntry::from).collect();
    let project = ProjectListEntry {
        uuid: format_uuid(record.uuid),
        title: record.title,
        scene_list,
    };

    Ok(HttpResponse::Ok().json(ProjectResponse {
        message: "Project saved successfully.".to_string(),
        success: true,
        project,
    }))
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
            title: scene.title,
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
            title: project.title,
            scene_list,
        })
    }
}

#[derive(serde_derive::Serialize)]
struct ProjectListResponse {
    message: String,
    success: bool,
    list: Vec<ProjectListEntry>,
}

async fn list(mut conn: Pool, user: User) -> Result<HttpResponse, actix_web::Error> {
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
    mut conn: Pool,
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
    mut conn: Pool,
    user: User,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, actix_web::Error> {
    let project = retrieve_uuid_from_path(path)?;
    let project = Project::get_by_uuid(conn.acquire(), project)
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
    mut conn: Pool,
    user: User,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, actix_web::Error> {
    let project = retrieve_uuid_from_path(path)?;
    let project = Project::get_by_uuid(conn.acquire(), project)
        .await
        .map_err(e500)?;

    if project.user != user.uuid {
        res_failure("Project not found.")
    } else {
        project.delete(conn.acquire()).await.map_err(e500)?;
        res_success("Project deleted successfully.")
    }
}
