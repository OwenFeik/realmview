use std::{collections::HashMap, sync::Arc};

use actix_web::{error::ErrorUnprocessableEntity, web, HttpRequest, HttpResponse};
use sqlx::SqlitePool;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::{res_failure, res_json, res_success, Resp};
use crate::{
    games::{close_ws, connect_client, launch_server, GameHandle, GameKey},
    models::{Project, Scene, User},
    req::e500,
};

type Games = RwLock<HashMap<GameKey, GameHandle>>;

pub fn routes() -> actix_web::Scope {
    web::scope("/game")
        .route("/new", web::post().to(new))
        .route("/{game_key}/end", web::post().to(end))
        .route("/{game_key}", web::post().to(test))
        .route("/{game_key}", web::get().to(join))
}

#[cfg_attr(test, derive(serde_derive::Serialize))]
#[derive(serde_derive::Deserialize)]
struct NewGameRequest {
    scene: Uuid,
}

#[cfg_attr(test, derive(serde_derive::Deserialize))]
#[derive(serde_derive::Serialize)]
struct GameResponse {
    message: String,
    success: bool,
    url: String,
}

fn game_url(game_key: &GameKey) -> String {
    format!("/game/{game_key}")
}

async fn new(
    pool: web::Data<SqlitePool>,
    games: web::Data<Games>,
    user: User,
    req: web::Json<NewGameRequest>,
) -> Resp {
    let conn = &mut pool.acquire().await.map_err(e500)?;
    let project = match Scene::get_by_uuid(conn, req.scene).await {
        Ok(r) => match Project::for_scene(conn, r.uuid).await {
            Ok(proj) => {
                if user.uuid == proj.user {
                    proj.load(conn).await.map_err(e500)?
                } else {
                    return res_failure("Scene owned by a different user.");
                }
            }
            _ => return res_failure("Project not found."),
        },
        _ => return res_failure("Scene not found."),
    };

    let game_key = {
        let lock = games.read().await;
        loop {
            let game_key = GameKey::new().map_err(e500)?;
            if !lock.contains_key(&game_key) {
                break game_key;
            }
        }
    };

    let pool = (*pool.into_inner()).clone();
    let server = launch_server(game_key.clone(), user, project, req.scene, pool);
    games.write().await.insert(game_key.clone(), server);
    let game_location = game_url(&game_key);
    let resp = HttpResponse::Ok()
        .insert_header(("location", game_location.as_str()))
        .json(GameResponse {
            message: "Game launched successfully.".to_string(),
            success: true,
            url: game_location,
        });
    Ok(resp)
}

async fn end(games: web::Data<Games>, user: User, path: web::Path<(String,)>) -> Resp {
    match GameKey::from(path.into_inner().0) {
        Ok(game_key) => {
            if let Some(handle) = games.read().await.get(&game_key) {
                if handle.owner == user.uuid {
                    handle.close();
                    res_success("Game closed.")
                } else {
                    res_failure("A game may only be closed by its owner.")
                }
            } else {
                res_success("Game already closed.")
            }
        }
        Err(e) => res_failure(e),
    }
}

async fn join_game(
    req: HttpRequest,
    stream: web::Payload,
    games: Arc<Games>,
    user: User,
    game_key: &GameKey,
) -> Resp {
    let (resp, mut session, msg_stream) = actix_ws::handle(&req, stream)?;

    match games.read().await.get(game_key) {
        Some(handle) => {
            connect_client(user, handle.clone(), session, msg_stream);
        }
        None => {
            // Just send a gameover message and close the socket.
            session
                .binary(bincode::serialize(&scene::comms::ServerEvent::GameOver).map_err(e500)?)
                .await
                .map_err(e500)?;

            close_ws(session).await;
        }
    };

    Ok(resp)
}

async fn join(
    req: HttpRequest,
    stream: web::Payload,
    games: web::Data<Games>,
    user: User,
    path: web::Path<(String,)>,
) -> Resp {
    let game_key = GameKey::from(path.into_inner().0).map_err(ErrorUnprocessableEntity)?;
    join_game(req, stream, games.into_inner(), user, &game_key).await
}

async fn test(games: web::Data<Games>, path: web::Path<(String,)>) -> Resp {
    let game_key = GameKey::from(path.into_inner().0).map_err(ErrorUnprocessableEntity)?;
    let url = game_url(&game_key);
    if let Some(handle) = games.read().await.get(&game_key) {
        if handle.open() {
            res_json(GameResponse {
                message: "Game exists.".to_string(),
                success: true,
                url,
            })
        } else {
            res_json(GameResponse {
                message: "Game has ended.".to_string(),
                success: false,
                url,
            })
        }
    } else {
        res_json(GameResponse {
            message: "Game doesn't exist.".to_string(),
            success: false,
            url,
        })
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use actix_web::{
        test::{self, TestRequest},
        web::Data,
    };

    use super::{GameResponse, Games, NewGameRequest};
    use crate::{
        api::{routes, Binary},
        games::{GameHandle, GameKey},
        models::{Project, User},
    };

    #[actix_web::test]
    async fn test_game_api() {
        // Test
        //   POST /api/game/new
        //   POST /api/game/{game_key}/end
        //   POST /api/{game_key}

        let db = crate::fs::initialise_database().await.unwrap();
        let games: Data<Games> = Data::new(tokio::sync::RwLock::new(
            HashMap::<GameKey, GameHandle>::new(),
        ));
        let app = test::init_service(
            actix_web::App::new()
                .app_data(Data::new(db.clone()))
                .app_data(games.clone())
                .service(routes()),
        )
        .await;

        let conn = &mut db.acquire().await.unwrap();
        let host = User::generate(conn).await;
        let project = Project::create(conn, &host, "project").await.unwrap();
        let mut proj = project.load(conn).await.unwrap();
        proj.new_scene();
        let (_, scenes) = Project::save(conn, &host, proj).await.unwrap();
        assert_eq!(scenes.len(), 1);
        let scene = scenes.first().unwrap().uuid;

        // Attempt to start a game with this project should fail for a user who
        // does not own the project.
        let other = User::generate(conn).await;
        let other_session = other.session(conn).await;
        let req = TestRequest::post()
            .uri("/api/game/new")
            .cookie(other_session.clone())
            .set_json(NewGameRequest { scene })
            .to_request();
        let resp: Binary = test::call_and_read_body_json(&app, req).await;
        assert!(!resp.success);

        let host_session = host.session(conn).await;
        let req = TestRequest::post()
            .uri("/api/game/new")
            .cookie(host_session.clone())
            .set_json(NewGameRequest { scene })
            .to_request();
        let resp: GameResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);
        assert_eq!(games.clone().read().await.len(), 1);

        // Response url is /game/{game_key}, which is used to serve the client
        // HTML before the client connects over the API. Skip that and just go
        // straight to the API.
        let client_url = resp.url;
        let url = format!("/api{}", client_url);

        // Non-host user should not be able to end the game.
        let req = TestRequest::post()
            .uri(&format!("{url}/end"))
            .cookie(other_session.clone())
            .to_request();
        let resp: Binary = test::call_and_read_body_json(&app, req).await;
        assert!(!resp.success);
        assert_eq!(games.clone().read().await.len(), 1);

        // Non-host user should be able to query the game status.
        let req = TestRequest::post()
            .uri(&url)
            .cookie(other_session.clone())
            .to_request();
        let resp: GameResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);
        assert_eq!(resp.url, client_url);

        // Host should be able to query the game status.
        let req = TestRequest::post()
            .uri(&url)
            .cookie(host_session.clone())
            .to_request();
        let resp: GameResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);
        assert_eq!(resp.url, client_url);

        // Host user should not be able to end the game.
        let req = TestRequest::post()
            .uri(&format!("{url}/end"))
            .cookie(host_session.clone())
            .to_request();
        let resp: Binary = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);
        assert!(!games.clone().read().await.values().next().unwrap().open());
    }
}
