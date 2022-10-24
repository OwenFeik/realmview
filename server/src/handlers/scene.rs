use std::path::Path;

use serde_derive::Serialize;
use sqlx::SqlitePool;
use warp::{hyper::StatusCode, Filter, Rejection, Reply};

use super::{with_db, with_session};
use crate::{
    handlers::response::{as_result, Binary, ResultReply},
    models::{Project, User},
};

pub const SCENE_EDITOR_FILE: &str = "scene.html";

pub fn routes(
    pool: sqlx::SqlitePool,
    content_dir: &Path,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("scene")
        .and(
            save::filter(pool.clone())
                .or(load::filter(pool.clone()))
                .or(delete_filter(pool)),
        )
        .or(proj_scene_route(content_dir))
}

fn proj_scene_route(
    content_dir: &Path,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("project" / String / "scene" / String)
        .map(|_proj_key, _scene_key| {}) // TODO could validate
        .untuple_one()
        .and(warp::get())
        .and(warp::fs::file(content_dir.join(SCENE_EDITOR_FILE)))
}

#[derive(Serialize)]
struct SceneResponse {
    message: String,
    project_title: String,
    project_key: String,
    project_id: i64,
    scene: String,
    scene_key: String,
    success: bool,
    title: String,
}

impl SceneResponse {
    fn reply(scene: scene::Scene, key: String, project: Project) -> ResultReply {
        let scene_str = match bincode::serialize(&scene) {
            Ok(b) => base64::encode(b),
            Err(_) => return Binary::result_error("Error encoding scene."),
        };

        as_result(
            &SceneResponse {
                message: "Scene saved.".to_string(),
                project_title: project.title,
                project_key: project.project_key,
                project_id: project.id,
                scene: scene_str,
                scene_key: key,
                success: true,
                title: scene.title.unwrap_or_else(|| "Untitled".to_string()),
            },
            StatusCode::OK,
        )
    }
}

mod save {
    use std::convert::Infallible;

    use serde_derive::Deserialize;
    use sqlx::SqlitePool;
    use warp::Filter;

    use crate::{
        handlers::{json_body, response::Binary, with_db, with_session},
        models::{Project, User},
    };

    const DEFAULT_SCENE_TITLE: &str = "Untitled";

    #[derive(Deserialize)]
    struct SceneSaveRequest {
        project_title: String,
        title: String,
        encoded: String,
    }

    async fn save_scene(
        pool: sqlx::SqlitePool,
        skey: String,
        req: SceneSaveRequest,
    ) -> Result<impl warp::Reply, Infallible> {
        let scene: scene::Scene = match base64::decode(req.encoded) {
            Ok(b) => match bincode::deserialize(&b) {
                Ok(s) => s,
                Err(_) => {
                    return Binary::result_failure(
                        "Deserialisation failure. Possible version mismatch.",
                    )
                }
            },
            Err(_) => return Binary::result_failure("Decoding failure."),
        };

        let user = match User::get_by_session(&pool, &skey).await {
            Ok(Some(u)) => u,
            _ => return Binary::result_failure("Invalid session."),
        };

        let conn = &mut match pool.acquire().await {
            Ok(c) => c,
            Err(e) => return Binary::result_error(&format!("{e}")),
        };

        let mut project = match Project::get_or_create(conn, scene.project, user.id).await {
            Ok(p) => p,
            Err(_) => return Binary::result_failure("Missing project."),
        };

        if project.title != req.project_title
            && matches!(project.update_title(conn, req.project_title).await, Err(_))
        {
            return Binary::result_failure("Failed to update project title.");
        }

        let mut scene_title = req.title.trim();
        if scene_title.is_empty() {
            scene_title = DEFAULT_SCENE_TITLE;
        }

        match project
            .update_scene(conn, scene, scene_title.to_string())
            .await
        {
            Ok(r) => match r.load_scene(conn).await {
                Ok(s) => super::SceneResponse::reply(s, r.scene_key, project),
                Err(s) => Binary::result_failure(&format!(
                    "Failed to load saved scene: {}",
                    &s.to_string()
                )),
            },
            Err(s) => Binary::result_failure(&format!("Failed to save scene: {}", &s.to_string())),
        }
    }

    #[derive(Deserialize)]
    struct SceneDetailsRequest {
        project_title: Option<String>,
        project_key: String,
        scene_title: Option<String>,
        scene_key: Option<String>,
    }

    async fn scene_details(
        pool: SqlitePool,
        skey: String,
        req: SceneDetailsRequest,
    ) -> Result<impl warp::Reply, Infallible> {
        let user = match User::get_by_session(&pool, &skey).await {
            Ok(Some(u)) => u,
            _ => return Binary::result_failure("Invalid session."),
        };

        let conn = &mut match pool.acquire().await {
            Ok(c) => c,
            Err(e) => return Binary::from_error(e),
        };

        let mut project = match Project::get_by_key(conn, &req.project_key).await {
            Ok(p) => p,
            Err(e) => return Binary::from_error(e),
        };

        if project.user != user.id {
            return Binary::result_failure("Project not found.");
        }

        if let Some(title) = req.project_title {
            if let Err(e) = project.update_title(conn, title).await {
                return Binary::from_error(e);
            }
        }

        if let (Some(title), Some(scene_key)) = (req.scene_title, req.scene_key) {
            if let Err(e) = project.update_scene_title(conn, &scene_key, &title).await {
                return Binary::from_error(e);
            }
        }

        Binary::result_success("Updated successfully")
    }

    pub fn filter(
        pool: sqlx::SqlitePool,
    ) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        (warp::path("save")
            .and(warp::post())
            .and(with_db(pool.clone()))
            .and(with_session())
            .and(json_body())
            .and_then(save_scene))
        .or(warp::path("details")
            .and(warp::post())
            .and(with_db(pool))
            .and(with_session())
            .and(json_body())
            .and_then(scene_details))
    }
}

mod load {
    use warp::Filter;

    use crate::{
        handlers::{response::Binary, with_db, with_session},
        models::{Project, SceneRecord, User},
    };

    async fn load_scene(
        scene_key: String,
        pool: sqlx::SqlitePool,
        skey: String,
    ) -> Result<impl warp::Reply, std::convert::Infallible> {
        let user = match User::get_by_session(&pool, &skey).await {
            Ok(Some(u)) => u,
            _ => return Binary::result_failure("Invalid session."),
        };

        let conn = &mut match pool.acquire().await {
            Ok(c) => c,
            Err(e) => return Binary::result_error(&format!("{e}")),
        };

        let record = match SceneRecord::load_from_key(conn, &scene_key).await {
            Ok(r) => r,
            Err(_) => return Binary::result_failure("Scene not found."),
        };

        let project = match Project::load(conn, record.project).await {
            Ok(p) => p,
            Err(_) => return Binary::result_failure("Project not found."),
        };

        if project.user != user.id {
            return Binary::result_failure("Project belongs to different user.");
        }

        match record.load_scene(conn).await {
            Ok(s) => super::SceneResponse::reply(s, scene_key, project),
            _ => Binary::result_failure("Failed to load scene."),
        }
    }

    pub fn filter(
        pool: sqlx::SqlitePool,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("load" / String)
            .and(warp::get())
            .and(with_db(pool))
            .and(with_session())
            .and_then(load_scene)
    }
}

async fn delete_scene(scene_key: String, pool: SqlitePool, skey: String) -> ResultReply {
    let user = match User::get_by_session(&pool, &skey).await {
        Ok(Some(u)) => u,
        _ => return Binary::result_failure("Invalid session."),
    };

    let conn = &mut match pool.acquire().await {
        Ok(c) => c,
        Err(e) => return Binary::result_error(&format!("{e}")),
    };

    match Project::delete_scene(conn, user.id, &scene_key).await {
        Ok(()) => Binary::result_success("Scene deleted successfullly."),
        Err(_) => Binary::result_failure("Scene not found."),
    }
}

fn delete_filter(pool: SqlitePool) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path!(String)
        .and(warp::path::end())
        .and(with_db(pool))
        .and(with_session())
        .and_then(delete_scene)
}
