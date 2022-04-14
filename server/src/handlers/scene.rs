use warp::Filter;

pub fn routes(
    pool: sqlx::SqlitePool,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    save::filter(pool)
}

mod save {
    // TODO needs to simply return the saved scene to the client so that all of
    // the relevant layers, sprites, etc have their canonical IDs preserved.
    // Current solution duplicates all these objects each time the scene is
    // saved, which is obviously wrong.

    use std::convert::Infallible;

    use serde_derive::Serialize;
    use warp::{hyper::StatusCode, Filter};

    use crate::{
        handlers::{
            binary_body,
            response::{as_result, Binary, ResultReply},
            with_db, with_session,
        },
        models::{Project, User},
    };

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
        scene: scene::Scene,
    ) -> Result<impl warp::Reply, Infallible> {
        let user = match User::get_by_session(&pool, &skey).await {
            Ok(Some(u)) => u,
            _ => return Binary::result_failure("Invalid session."),
        };

        let project = match Project::get_or_create(&pool, scene.project, user.id).await {
            Ok(p) => p,
            Err(_) => return Binary::result_failure("Missing project."),
        };

        match project.update_scene(&pool, scene).await {
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
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("scene" / "save")
            .and(warp::post())
            .and(with_db(pool))
            .and(with_session())
            .and(binary_body())
            .and_then(save_scene)
    }
}
