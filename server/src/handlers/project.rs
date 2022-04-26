use sqlx::SqlitePool;
use warp::Filter;

use crate::models::{Project, User};

use super::{
    response::{Binary, ResultReply},
    with_db, with_session,
};

struct SceneListEntry {
    scene_key: String,
    title: String,
    n_sprites: i64,
}

struct ProjectListEntry {
    project_key: String,
    title: String,
    scene_list: Vec<SceneListEntry>,
}

struct ProjectListResponse {
    message: String,
    success: bool,
    list: Vec<ProjectListEntry>,
}

async fn list_projects(pool: SqlitePool, session_key: String) -> ResultReply {
    let user = match User::get_by_session(&pool, &session_key).await {
        Ok(Some(user)) => user,
        _ => return Binary::result_failure("Invalid session."),
    };

    let projects = match Project::list(&pool, user.id).await {
        Ok(v) => v,
        Err(_) => return Binary::result_failure("Failed to retrieve project list."),
    };

    super::response::Binary::result_success("Ok")
}

pub fn filter(
    pool: SqlitePool,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("project" / "list")
        .and(warp::get())
        .and(with_db(pool))
        .and(with_session())
        .and_then(list_projects)
}
