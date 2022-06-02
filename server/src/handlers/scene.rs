use serde_derive::Serialize;
use warp::{hyper::StatusCode, Filter};

use crate::handlers::response::{as_result, Binary, ResultReply};

pub fn routes(
    pool: sqlx::SqlitePool,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    save::filter(pool.clone()).or(load::filter(pool))
}

#[derive(Serialize)]
struct SceneResponse {
    message: String,
    scene: String,
    success: bool,
    title: String,
}

impl SceneResponse {
    fn reply(scene: scene::Scene) -> ResultReply {
        let scene_str = match bincode::serialize(&scene) {
            Ok(b) => base64::encode(b),
            Err(_) => return Binary::result_error("Error encoding scene."),
        };

        as_result(
            &SceneResponse {
                message: "Scene saved.".to_string(),
                scene: scene_str,
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
    use warp::Filter;

    use crate::{
        handlers::{json_body, response::Binary, with_db, with_session},
        models::{Project, User},
    };

    const DEFAULT_SCENE_TITLE: &str = "Untitled";

    #[derive(Deserialize)]
    struct SceneSaveRequest {
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
                Err(_) => return Binary::result_failure("Deserialisation failure."),
            },
            Err(_) => return Binary::result_failure("Decoding failure."),
        };

        let user = match User::get_by_session(&pool, &skey).await {
            Ok(Some(u)) => u,
            _ => return Binary::result_failure("Invalid session."),
        };

        let project = match Project::get_or_create(&pool, scene.project, user.id).await {
            Ok(p) => p,
            Err(_) => return Binary::result_failure("Missing project."),
        };

        let mut scene_title = req.title.trim();
        if scene_title.is_empty() {
            scene_title = DEFAULT_SCENE_TITLE;
        }

        match project
            .update_scene(&pool, scene, scene_title.to_string())
            .await
        {
            Ok(s) => match s.load_scene(&pool).await {
                Ok(s) => super::SceneResponse::reply(s),
                Err(s) => Binary::result_failure(&format!(
                    "Failed to load saved scene: {}",
                    &s.to_string()
                )),
            },
            Err(s) => Binary::result_failure(&format!("Failed to save scene: {}", &s.to_string())),
        }
    }

    pub fn filter(
        pool: sqlx::SqlitePool,
    ) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("scene" / "save")
            .and(warp::post())
            .and(with_db(pool))
            .and(with_session())
            .and(json_body())
            .and_then(save_scene)
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

        let record = match SceneRecord::load_from_key(&pool, scene_key).await {
            Ok(r) => r,
            Err(_) => return Binary::result_failure("Scene not found."),
        };

        let project = match Project::load(&pool, record.project).await {
            Ok(p) => p,
            Err(_) => return Binary::result_failure("Project not found."),
        };

        if project.user != user.id {
            return Binary::result_failure("Project belongs to different user.");
        }

        match record.load_scene(&pool).await {
            Ok(s) => super::SceneResponse::reply(s),
            _ => Binary::result_failure("Failed to load scene."),
        }
    }

    pub fn filter(
        pool: sqlx::SqlitePool,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("scene" / "load" / String)
            .and(warp::get())
            .and(with_db(pool))
            .and(with_session())
            .and_then(load_scene)
    }
}
