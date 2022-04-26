pub fn routes(
    pool: sqlx::SqlitePool,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    save::filter(pool)
}

mod save {
    // TODO needs to simply return the saved scene to the client so that all of
    // the relevant layers, sprites, etc have their canonical IDs preserved.
    // Current solution duplicates all these objects each time the scene is
    // saved, which is obviously wrong.

    use std::convert::Infallible;

    use serde_derive::{Deserialize, Serialize};
    use warp::{hyper::StatusCode, Filter};

    use crate::{
        handlers::{
            json_body,
            response::{as_result, Binary, ResultReply},
            with_db, with_session,
        },
        models::{Project, User},
    };

    const DEFAULT_SCENE_TITLE: &str = "Untitled";

    #[derive(Deserialize)]
    struct SceneSaveRequest {
        title: String,
        encoded: String,
    }

    #[derive(Serialize)]
    struct SceneSaveResponse {
        message: String,
        scene: String,
        success: bool,
    }

    impl SceneSaveResponse {
        fn reply(scene: scene::Scene) -> ResultReply {
            let scene_str = match bincode::serialize(&scene) {
                Ok(b) => base64::encode(b),
                Err(_) => return Binary::result_error("Error encoding scene."),
            };

            as_result(
                &SceneSaveResponse {
                    message: "Scene saved.".to_string(),
                    scene: scene_str,
                    success: true,
                },
                StatusCode::OK,
            )
        }
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
                Ok(s) => SceneSaveResponse::reply(s),
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
