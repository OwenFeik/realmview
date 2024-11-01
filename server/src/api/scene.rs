use actix_web::{web, HttpResponse};

use super::{res_failure, res_success, res_unproc, Resp};
use crate::models::{Project, Scene, User};
use crate::req::{e500, Conn};

pub fn routes() -> actix_web::Scope {
    web::scope("/scene")
        .route("/save", web::post().to(save))
        .route("/details", web::post().to(details))
        .route("/load/{scene_key}", web::get().to(load))
        .route("/{scene_key}", web::post().to(save))
        .route("/{scene_key}", web::delete().to(delete))
}

#[derive(serde_derive::Serialize)]
struct SceneResponse {
    message: String,
    project_title: String,
    project_uuid: String,
    scene: String,
    scene_key: String,
    success: bool,
    title: String,
}

impl SceneResponse {
    const DEFAULT_TITLE: &'static str = "Untitled";

    fn reply(scene: scene::Scene, key: String, project: Project) -> Resp {
        let scene_raw = bincode::serialize(&scene).map_err(e500)?;
        Ok(HttpResponse::Ok().json(&SceneResponse {
            message: "Scene saved.".to_string(),
            project_title: project
                .title
                .unwrap_or_else(|| Self::DEFAULT_TITLE.to_string()),
            project_uuid: project.uuid.simple().to_string(),
            scene: base64::encode(scene_raw),
            scene_key: key,
            success: true,
            title: scene
                .title
                .unwrap_or_else(|| Self::DEFAULT_TITLE.to_string()),
        }))
    }
}

#[derive(serde_derive::Deserialize)]
struct SceneSaveRequest {
    encoded: String,
}

async fn save(mut conn: Conn, user: User, req: web::Json<SceneSaveRequest>) -> Resp {
    let Ok(Ok(scene)) =
        base64::decode(&req.encoded).map(|bytes| bincode::deserialize::<scene::Scene>(&bytes))
    else {
        return res_unproc("Failed to decode scene.");
    };

    let project = Project::get_or_create(conn.acquire(), scene.project, user.id)
        .await
        .map_err(e500)?;

    let record = project
        .update_scene(conn.acquire(), scene)
        .await
        .map_err(e500)?;
    let scene = record.load_scene(conn.acquire()).await.map_err(e500)?;

    SceneResponse::reply(scene, record.scene_key, project)
}

#[derive(serde_derive::Deserialize)]
struct SceneDetailsRequest {
    project_title: Option<String>,
    project_key: String,
    scene_title: Option<String>,
    scene_key: Option<String>,
}

async fn details(mut conn: Conn, user: User, req: web::Json<SceneDetailsRequest>) -> Resp {
    let mut project = Project::get_by_key(conn.acquire(), &req.project_key)
        .await
        .map_err(e500)?;

    if project.user != user.id {
        return res_failure("Project not found.");
    }

    if let Some(title) = &req.project_title {
        project
            .update_title(conn.acquire(), title.clone())
            .await
            .map_err(e500)?;
    }

    if let (Some(title), Some(scene_key)) = (&req.scene_title, &req.scene_key) {
        project
            .update_scene_title(conn.acquire(), scene_key, title)
            .await
            .map_err(e500)?;
    }

    res_success("Updated successfully.")
}

async fn load(mut conn: Conn, user: User, path: web::Path<(String,)>) -> Resp {
    let scene_key = path.into_inner().0;
    let record = SceneRecord::load_from_key(conn.acquire(), &scene_key)
        .await
        .map_err(e500)?;
    let project = Project::load(conn.acquire(), record.project)
        .await
        .map_err(e500)?;

    if project.user != user.id {
        res_failure("Project not found.")
    } else {
        SceneResponse::reply(
            record.load_scene(conn.acquire()).await.map_err(e500)?,
            scene_key,
            project,
        )
    }
}

async fn delete(mut conn: Conn, user: User, path: web::Path<(String,)>) -> Resp {
    let scene_key = path.into_inner().0;
    match Project::delete_scene(conn.acquire(), user.id, &scene_key).await {
        Ok(_) => res_success("Scene deleted successfullly."),
        Err(_) => res_failure("Scene not found."),
    }
}
