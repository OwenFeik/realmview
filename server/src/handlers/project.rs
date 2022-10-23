use serde_derive::Serialize;
use sqlx::SqlitePool;
use warp::{hyper::StatusCode, Filter};

use super::{
    response::{as_result, Binary, ResultReply},
    with_db, with_session,
};
use crate::models::{Project, User};

#[derive(Serialize)]
struct SceneListEntry {
    scene_key: String,
    title: String,
    thumbnail: String,
}

#[derive(Serialize)]
struct ProjectListEntry {
    id: i64,
    project_key: String,
    title: String,
    scene_list: Vec<SceneListEntry>,
}

#[derive(Serialize)]
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

    let conn = &mut match pool.acquire().await {
        Ok(c) => c,
        Err(e) => return Binary::result_error(&format!("{e}")),
    };

    let mut projects = match Project::list(conn, user.id).await {
        Ok(v) => v,
        Err(_) => return Binary::result_failure("Failed to retrieve project list."),
    };

    let mut project_list = vec![];
    while let Some(project) = projects.pop() {
        let scene_list = match project.list_scenes(conn).await {
            Ok(scenes) => scenes
                .into_iter()
                .map(|s| SceneListEntry {
                    scene_key: s.scene_key,
                    title: s.title,
                    thumbnail: s.thumbnail,
                })
                .collect(),
            Err(e) => return Binary::result_error(&format!("Database error. {e}")),
        };
        project_list.push(ProjectListEntry {
            id: project.id,
            project_key: project.project_key,
            title: project.title,
            scene_list,
        });
    }

    as_result(
        &ProjectListResponse {
            message: "Project list retrieved.".to_string(),
            success: true,
            list: project_list,
        },
        StatusCode::OK,
    )
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
