use actix_web::error::{ErrorNotFound, ErrorUnprocessableEntity};
use actix_web::{error::ErrorInternalServerError, web, HttpResponse};
use scene::requests::{ProjectListEntry, SceneListEntry};
use sqlx::SqliteConnection;
use uuid::Uuid;

use super::{res_failure, res_json, res_success, res_unproc, resp_json};
use crate::models::{Project, Scene, User};
use crate::req::{e500, Pool};
use crate::utils::{format_uuid, Res};

pub fn routes() -> actix_web::Scope {
    web::scope("/project")
        .route("/save", web::post().to(save))
        .route("/list", web::get().to(list))
        .route("/new", web::post().to(new))
        .route("/{uuid}", web::get().to(info))
        .route("/{uuid}", web::patch().to(edit_details))
        .route("/{uuid}/save", web::get().to(get))
        .route("/{uuid}", web::delete().to(delete))
}

async fn save(
    mut pool: Pool,
    user: User,
    body: bytes::Bytes,
) -> Result<HttpResponse, actix_web::Error> {
    let conn = pool.acquire();

    let Ok(project) = scene::serde::deserialise(&body) else {
        return res_unproc("Failed to decode project.");
    };

    let scene_list = project
        .scenes
        .iter()
        .map(|s| SceneListEntry {
            uuid: format_uuid(s.uuid),
            title: s.title.clone(),
            updated_time: 0,
            thumbnail: None,
        })
        .collect();
    let project_old = ProjectListEntry {
        uuid: format_uuid(project.uuid),
        title: project.title.clone(),
        updated_time: 0,
        scene_list,
    };

    let (record, scenes) = match Project::save(conn, &user, project).await {
        Ok(record) => record,
        Err(e) => return Err(e500(e)),
    };

    let updated_time = record.updated_timestamp();
    let scene_list = scenes.into_iter().map(scene_list_entry).collect();
    let project_new = ProjectListEntry {
        uuid: format_uuid(record.uuid),
        title: record.title,
        updated_time,
        scene_list,
    };

    let resp = HttpResponse::Ok().json(scene::requests::ProjectSaveResponse {
        message: "Project saved successfully.".to_string(),
        success: true,
        project_old,
        project_new,
    });
    Ok(resp)
}

fn scene_list_entry(scene: Scene) -> SceneListEntry {
    let updated_time = scene.updated_timestamp();
    SceneListEntry {
        uuid: format_uuid(scene.uuid),
        title: scene.title,
        updated_time,
        thumbnail: scene.thumbnail,
    }
}

async fn project_list_entry(
    project: Project,
    conn: &mut SqliteConnection,
) -> Res<ProjectListEntry> {
    let scene_list = project
        .list_scenes(conn)
        .await?
        .into_iter()
        .map(scene_list_entry)
        .collect();
    let updated_time = project.updated_timestamp();
    Ok(ProjectListEntry {
        uuid: format_uuid(project.uuid),
        title: project.title,
        updated_time,
        scene_list,
    })
}

async fn list(mut conn: Pool, user: User) -> Result<HttpResponse, actix_web::Error> {
    let mut projects = Project::list_for_user(conn.acquire(), user.uuid)
        .await
        .map_err(ErrorInternalServerError)?;

    let mut project_list = vec![];
    while let Some(project) = projects.pop() {
        let entry = project_list_entry(project, conn.acquire())
            .await
            .map_err(e500)?;
        project_list.push(entry);
    }

    Ok(
        HttpResponse::Ok().json(&scene::requests::ProjectListResponse {
            message: "Project list retrieved.".to_string(),
            success: true,
            list: project_list,
        }),
    )
}

#[cfg_attr(test, derive(serde_derive::Serialize))]
#[derive(serde_derive::Deserialize)]
struct NewProjectRequest {
    title: String,
}

#[cfg_attr(test, derive(serde_derive::Deserialize))]
#[derive(serde_derive::Serialize)]
struct NewProjectResponse {
    message: String,
    success: bool,
    uuid: String,
    title: String,
    url: String,
}

async fn new(
    mut conn: Pool,
    user: User,
    req: web::Json<NewProjectRequest>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Err(e) = Project::validate_title(&req.title) {
        return Err(ErrorUnprocessableEntity(e));
    }

    let project = Project::create(conn.acquire(), &user, &req.title)
        .await
        .map_err(e500)?;

    let uuid = format_uuid(project.uuid);
    let url = format!("/project/{}", &uuid);
    res_json(NewProjectResponse {
        message: "Project created successfully.".to_owned(),
        success: true,
        uuid,
        title: req.title.clone(),
        url,
    })
}

fn retrieve_uuid_from_path(path: web::Path<(String,)>) -> Result<Uuid, actix_web::Error> {
    Uuid::try_parse(&path.into_inner().0)
        .map_err(|e| ErrorUnprocessableEntity(format!("Invalid UUID: {e}")))
}

#[cfg_attr(test, derive(serde_derive::Deserialize))]
#[derive(serde_derive::Serialize)]
struct ProjectInfoResponse {
    success: bool,
    message: String,
    project: ProjectListEntry,
}

async fn info(
    mut conn: Pool,
    user: User,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, actix_web::Error> {
    let project = match Project::lookup(conn.acquire(), retrieve_uuid_from_path(path)?).await {
        Ok(Some(record)) => record,
        Ok(None) => return Err(ErrorNotFound("Project does not exist.")),
        Err(e) => return Err(e500(e)),
    };

    if project.user != user.uuid {
        return res_failure("Project not found.");
    }

    res_json(ProjectInfoResponse {
        success: true,
        message: "Project info follows.".to_string(),
        project: project_list_entry(project, conn.acquire())
            .await
            .map_err(e500)?,
    })
}

async fn get(
    mut conn: Pool,
    user: User,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, actix_web::Error> {
    let project = match Project::lookup(conn.acquire(), retrieve_uuid_from_path(path)?).await {
        Ok(Some(record)) => record,
        Ok(None) => return Err(ErrorNotFound("Project does not exist.")),
        Err(e) => return Err(e500(e)),
    };

    if project.user != user.uuid {
        return res_failure("Project not found.");
    }

    let data = project.load_file(conn.acquire()).await.map_err(e500)?;
    Ok(HttpResponse::Ok().body(data))
}

async fn delete(
    mut conn: Pool,
    user: User,
    path: web::Path<(String,)>,
) -> Result<HttpResponse, actix_web::Error> {
    let project = match Project::lookup(conn.acquire(), retrieve_uuid_from_path(path)?).await {
        Ok(Some(record)) => record,
        Ok(None) => return Err(ErrorNotFound("Project does not exist.")),
        Err(e) => return Err(e500(e)),
    };

    if project.user != user.uuid {
        res_failure("Project not found.")
    } else {
        project.delete(conn.acquire(), &user).await.map_err(e500)?;
        res_success("Project deleted successfully.")
    }
}

#[cfg_attr(test, derive(serde_derive::Serialize))]
#[derive(serde_derive::Deserialize)]
struct ProjectDetailsRequest {
    title: String,
}

async fn edit_details(
    mut conn: Pool,
    user: User,
    req: web::Json<ProjectDetailsRequest>,
    path: web::Path<(Uuid,)>,
) -> Result<HttpResponse, actix_web::Error> {
    let conn = conn.acquire();
    let project = Project::get_by_uuid(conn, path.into_inner().0)
        .await
        .map_err(ErrorNotFound)?;

    if project.user != user.uuid {
        return Err(ErrorNotFound("Project not found."));
    }

    // Update title and save project.
    let mut project = project.load(conn).await.map_err(e500)?;
    project.title = req.into_inner().title;
    Project::save(conn, &user, project)
        .await
        .map(|(proj, scenes)| ProjectInfoResponse {
            success: true,
            message: "Project info follows".to_string(),
            project: ProjectListEntry {
                uuid: format_uuid(proj.uuid),
                title: proj.title.clone(),
                updated_time: proj.updated_timestamp(),
                scene_list: scenes.into_iter().map(scene_list_entry).collect(),
            },
        })
        .map(resp_json)
        .map_err(e500)
}

#[cfg(test)]
mod test {
    use actix_web::{
        cookie::Cookie,
        http::StatusCode,
        test::{self, TestRequest},
    };
    use scene::requests::*;

    use super::{
        NewProjectRequest, NewProjectResponse, ProjectDetailsRequest, ProjectInfoResponse,
    };
    use crate::{
        api::Binary,
        models::{User, UserAuth},
        utils::{format_uuid, generate_uuid},
    };

    #[actix_web::test]
    async fn test_project_api() {
        // Test
        //   POST /api/project/new
        //   GET /api/project/list
        //   GET /api/project/{uuid}
        //   POST /api/project/save
        //   DELETE /api/project/{uuid}

        let db = crate::fs::initialise_database().await.unwrap();
        let app = test::init_service(
            actix_web::App::new()
                .app_data(actix_web::web::Data::new(db.clone()))
                .service(crate::api::routes()),
        )
        .await;

        // Invalid request; no sesion, no request body. Redirects to login.
        let req = TestRequest::post().uri("/api/project/new").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_redirection());

        // Log in.
        let conn = &mut db.acquire().await.unwrap();
        let user = User::generate(conn).await;
        let req = TestRequest::post()
            .uri("/api/auth/login")
            .append_header(("Content-Type", "application/json"))
            .set_payload(format!(
                r#"{{"username":"{}","password":"{}"}}"#,
                &user.username,
                UserAuth::GENERATED_USER_PASSWORD
            ))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let session =
            Cookie::parse(resp.headers().get("Set-Cookie").unwrap().to_str().unwrap()).unwrap();

        // Invalid request; no request body.
        let req = TestRequest::post()
            .uri("/api/project/new")
            .cookie(session.clone())
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_client_error());

        // Valid request, should succeed and create a project.
        let title = "My Project".to_string();
        let req = TestRequest::post()
            .uri("/api/project/new")
            .cookie(session.clone())
            .set_json(NewProjectRequest {
                title: title.clone(),
            })
            .to_request();
        let resp: NewProjectResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);
        assert_eq!(&resp.title, &title);
        let project = resp.uuid;

        // No session; should redirect to login.
        let req = TestRequest::get().uri("/api/project/list").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_redirection());

        // Request with session, should return list with project we created.
        let req = TestRequest::get()
            .uri("/api/project/list")
            .cookie(session.clone())
            .to_request();
        let resp: ProjectListResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);
        assert_eq!(resp.list.len(), 1);
        let record = resp.list.first().unwrap();
        assert_eq!(record.uuid, project);
        assert!(record.scene_list.is_empty());

        // Try to get a project which doesn't exist.
        let req = TestRequest::get()
            .uri(&format!("/api/project/{}", format_uuid(generate_uuid())))
            .cookie(session.clone())
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        // Try to get our project.
        let req = TestRequest::get()
            .uri(&format!("/api/project/{}", project))
            .cookie(session.clone())
            .to_request();
        let resp: ProjectInfoResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);
        assert_eq!(resp.project.uuid, project);
        assert_eq!(resp.project.title, title);
        assert_eq!(resp.project.scene_list.len(), 0);

        // Set the project's title and check it's updated.
        let title = "New Title".to_string();
        let req = TestRequest::patch()
            .uri(&format!("/api/project/{project}"))
            .cookie(session.clone())
            .set_json(ProjectDetailsRequest {
                title: title.clone(),
            })
            .to_request();
        let resp: ProjectInfoResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);
        assert_eq!(resp.project.title, title);

        // Try to load a project we created.
        let req = TestRequest::get()
            .uri(&format!("/api/project/{project}/save"))
            .cookie(session.clone())
            .to_request();
        let resp: bytes::Bytes = test::call_and_read_body(&app, req).await;
        let mut decoded = scene::serde::deserialise(&resp).unwrap();
        assert_eq!(format_uuid(decoded.uuid), project);
        assert_eq!(&decoded.title, &title);
        assert_eq!(decoded.scenes.len(), 0);

        // Create some scenes in the new project.
        let mut scene = decoded.new_scene().clone();
        scene.title = "First Scene".to_string();
        scene.new_sprite(None, scene.first_layer());
        scene.new_sprite(None, scene.first_background_layer());
        decoded.update_scene(scene).unwrap();
        let mut scene = decoded.new_scene().clone();
        scene.title = "Second Scene".to_string();
        scene.new_sprite(None, scene.layers.last().unwrap().id);
        scene.new_sprite(None, scene.layers.last().unwrap().id);
        decoded.update_scene(scene).unwrap();

        // Save the updated project.
        let data = scene::serde::serialise(&decoded).unwrap();
        let req = TestRequest::post()
            .uri("/api/project/save")
            .cookie(session.clone())
            .set_payload(data)
            .to_request();
        let resp: ProjectResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);
        assert_eq!(&resp.project.uuid, &project);
        assert_eq!(resp.project.scene_list.len(), 2);
        assert_eq!(
            resp.project.scene_list.first().unwrap().title,
            "First Scene"
        );

        // Delete the project.
        let req = TestRequest::delete()
            .uri(&format!("/api/project/{}", project))
            .cookie(session.clone())
            .to_request();
        let resp: Binary = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);

        // Project list should be empty.
        let req = TestRequest::get()
            .uri("/api/project/list")
            .cookie(session)
            .to_request();
        let resp: ProjectListResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);
        assert_eq!(resp.list.len(), 0);
    }
}
