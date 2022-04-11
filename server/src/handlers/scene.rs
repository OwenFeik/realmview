use warp::Filter;

pub fn routes(
    pool: sqlx::SqlitePool,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    save::filter(pool)
}

mod save {
    use std::convert::Infallible;

    use warp::Filter;

    use crate::{
        handlers::{binary_body, response::Binary, with_db, with_session},
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

        match project.update_scene(&pool, scene).await {
            Ok(()) => Binary::result_success("Scene saved."),
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
