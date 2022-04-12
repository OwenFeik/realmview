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
            response::{as_result, Binary},
            with_db, with_session,
        },
        models::{Project, User},
    };

    #[derive(Serialize)]
    struct SceneSaveResponse {
        message: String,
        project_id: i64,
        scene_id: i64,
        success: bool,
    }

    impl SceneSaveResponse {
        fn new(project_id: i64, scene_id: i64) -> Self {
            SceneSaveResponse {
                message: "Scene saved.".to_string(),
                project_id,
                scene_id,
                success: true,
            }
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
            Ok(id) => as_result(&SceneSaveResponse::new(project.id, id), StatusCode::OK),
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
