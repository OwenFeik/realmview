use serde_derive::{Deserialize, Serialize};
use sqlx::SqlitePool;
use warp::{hyper::StatusCode, Filter};

use super::{
    json_body,
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

#[derive(Deserialize)]
struct NewProjectRequest {
    title: String,
}

#[derive(Serialize)]
struct NewProjectResponse {
    message: String,
    success: bool,
    project_key: String,
    title: String,
}

async fn list_projects(pool: SqlitePool, session_key: String) -> ResultReply {
    let user = match User::get_by_session(&pool, &session_key).await {
        Ok(Some(user)) => user,
        _ => return Binary::result_failure("Invalid session."),
    };

    let conn = &mut match pool.acquire().await {
        Ok(c) => c,
        Err(e) => return Binary::from_error(e),
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

async fn delete_project(project_key: String, pool: SqlitePool, session_key: String) -> ResultReply {
    let user = match User::get_by_session(&pool, &session_key).await {
        Ok(Some(user)) => user,
        _ => return Binary::result_failure("Invalid session."),
    };

    let conn = &mut match pool.acquire().await {
        Ok(c) => c,
        Err(e) => return Binary::from_error(e),
    };

    let project = match Project::get_by_key(conn, &project_key).await {
        Ok(p) => p,
        Err(_) => return Binary::result_failure(""),
    };

    if project.user != user.id {
        return Binary::result_failure("Project not found.");
    }

    match project.delete(conn).await {
        Ok(()) => Binary::result_success("Project deleted successfully."),
        Err(e) => Binary::from_error(e),
    }
}

async fn new_project(
    request: NewProjectRequest,
    pool: SqlitePool,
    session_key: String,
) -> ResultReply {
    let user = match User::get_by_session(&pool, &session_key).await {
        Ok(Some(user)) => user,
        _ => return Binary::result_failure("Invalid session."),
    };

    let conn = &mut match pool.acquire().await {
        Ok(c) => c,
        Err(e) => return Binary::from_error(e),
    };

    let project = match Project::new(conn, user.id, &request.title).await {
        Ok(project) => project,
        Err(e) => return Binary::from_error(e),
    };

    as_result(
        &NewProjectResponse {
            message: "Project created successfully.".to_owned(),
            success: true,
            project_key: project.project_key,
            title: request.title,
        },
        StatusCode::OK,
    )
}

pub fn filter(
    pool: SqlitePool,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("project").and(
        (warp::path("list")
            .and(warp::get())
            .and(with_db(pool.clone()))
            .and(with_session())
            .and_then(list_projects))
        .or(warp::path("new")
            .and(warp::post())
            .and(json_body::<NewProjectRequest>())
            .and(with_db(pool.clone()))
            .and(with_session())
            .and_then(new_project))
        .or(warp::path!(String)
            .and(warp::path::end())
            .and(warp::delete())
            .and(with_db(pool))
            .and(with_session())
            .and_then(delete_project)),
    )
}
