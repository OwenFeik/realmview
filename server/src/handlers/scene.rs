mod save {
    use std::convert::Infallible;

    use warp::Filter;

    use crate::{
        handlers::{json_body, response::Binary, with_db, with_session},
        models::{Project, User},
    };

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

        Binary::result_success("Scene saved.")
    }

    pub fn filter(
        pool: sqlx::SqlitePool,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("scene")
            .and(warp::post())
            .and(with_db(pool))
            .and(with_session())
            .and(json_body())
            .and_then(save_scene)
    }
}
