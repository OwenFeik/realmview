use actix_web::error::{ErrorNotFound, ErrorUnprocessableEntity};
use actix_web::{error::ErrorInternalServerError, web, HttpResponse};
use sqlx::SqliteConnection;
use uuid::Uuid;

use super::{res_failure, res_json, res_success, res_unproc};
use crate::models::{Project, Scene, User};
use crate::req::{e500, Pool};
use crate::utils::{format_uuid, Res};

pub fn routes() -> actix_web::Scope {
    web::scope("/project")
        .route("/save", web::post().to(save))
        .route("/list", web::get().to(list))
        .route("/new", web::post().to(new))
        .route("/{uuid}", web::get().to(get))
        .route("/{uuid}", web::delete().to(delete))
}

#[cfg_attr(test, derive(serde_derive::Deserialize))]
#[derive(serde_derive::Serialize)]
struct ProjectResponse {
    message: String,
    success: bool,
    project: ProjectListEntry,
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

    let (record, scenes) = match Project::save(conn, &user, project).await {
        Ok(record) => record,
        Err(e) => return Err(e500(e)),
    };

    let scene_list = scenes.into_iter().map(SceneListEntry::from).collect();
    let project = ProjectListEntry {
        uuid: format_uuid(record.uuid),
        title: record.title,
        scene_list,
    };

    Ok(HttpResponse::Ok().json(ProjectResponse {
        message: "Project saved successfully.".to_string(),
        success: true,
        project,
    }))
}

#[cfg_attr(test, derive(serde_derive::Deserialize))]
#[derive(serde_derive::Serialize)]
struct SceneListEntry {
    uuid: String,
    title: String,
    updated_time: u64,
    thumbnail: Option<String>,
}

impl SceneListEntry {
    fn from(scene: Scene) -> Self {
        let updated_time = scene.updated_timestamp();
        SceneListEntry {
            uuid: format_uuid(scene.uuid),
            title: scene.title,
            updated_time,
            thumbnail: scene.thumbnail,
        }
    }
}

#[cfg_attr(test, derive(serde_derive::Deserialize))]
#[derive(serde_derive::Serialize)]
struct ProjectListEntry {
    uuid: String,
    title: String,
    scene_list: Vec<SceneListEntry>,
}

impl ProjectListEntry {
    async fn from(project: Project, conn: &mut SqliteConnection) -> Res<Self> {
        let scene_list = project
            .list_scenes(conn)
            .await?
            .into_iter()
            .map(SceneListEntry::from)
            .collect();
        Ok(ProjectListEntry {
            uuid: format_uuid(project.uuid),
            title: project.title,
            scene_list,
        })
    }
}

#[cfg_attr(test, derive(serde_derive::Deserialize))]
#[derive(serde_derive::Serialize)]
struct ProjectListResponse {
    message: String,
    success: bool,
    list: Vec<ProjectListEntry>,
}

async fn list(mut conn: Pool, user: User) -> Result<HttpResponse, actix_web::Error> {
    let mut projects = Project::list_for_user(conn.acquire(), user.uuid)
        .await
        .map_err(ErrorInternalServerError)?;

    let mut project_list = vec![];
    while let Some(project) = projects.pop() {
        let entry = ProjectListEntry::from(project, conn.acquire())
            .await
            .map_err(e500)?;
        project_list.push(entry);
    }

    Ok(HttpResponse::Ok().json(&ProjectListResponse {
        message: "Project list retrieved.".to_string(),
        success: true,
        list: project_list,
    }))
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

    res_json(NewProjectResponse {
        message: "Project created successfully.".to_owned(),
        success: true,
        uuid: format_uuid(project.uuid),
        title: req.title.clone(),
    })
}

fn retrieve_uuid_from_path(path: web::Path<(String,)>) -> Result<Uuid, actix_web::Error> {
    Uuid::try_parse(&path.into_inner().0)
        .map_err(|e| ErrorUnprocessableEntity(format!("Invalid UUID: {e}")))
}

#[cfg_attr(test, derive(serde::Deserialize))]
#[derive(serde::Serialize)]
struct ProjectDataResponse {
    uuid: String,
    title: String,
    project: Vec<u8>,
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
    res_json(ProjectDataResponse {
        uuid: format_uuid(project.uuid),
        title: project.title,
        project: data,
    })
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

#[cfg(test)]
mod test {
    use actix_web::{
        cookie::Cookie,
        http::StatusCode,
        test::{self, TestRequest},
    };

    use super::{
        NewProjectRequest, NewProjectResponse, ProjectDataResponse, ProjectListResponse,
        ProjectResponse,
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

        // Try to get project we created.
        let req = TestRequest::get()
            .uri(&format!("/api/project/{}", project))
            .cookie(session.clone())
            .to_request();
        let resp: ProjectDataResponse = test::call_and_read_body_json(&app, req).await;
        assert_eq!(resp.uuid, project);
        assert_eq!(&resp.title, &title);
        let mut decoded = scene::serde::deserialise(&resp.project).unwrap();
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
