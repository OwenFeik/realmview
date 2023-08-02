use std::{collections::HashMap, sync::Arc};

use actix_web::{web, HttpRequest, HttpResponse};
use sqlx::SqlitePool;
use tokio::sync::RwLock;

use super::{e500, res_failure, Res};
use crate::{
    games::{connect_client, generate_game_key, launch_server, GameHandle},
    models::{SceneRecord, User},
};

type Games = RwLock<HashMap<String, GameHandle>>;

pub fn routes() -> actix_web::Scope {
    web::scope("/game")
        .route("/new", web::post().to(new))
        .route("/{game_key}", web::to(join))
}

#[derive(serde_derive::Deserialize)]
struct NewGameRequest {
    scene_key: String,
}

#[derive(serde_derive::Serialize)]
struct NewGameResponse {
    message: String,
    success: bool,
    url: String,
}

async fn new(
    pool: web::Data<SqlitePool>,
    games: web::Data<Games>,
    user: User,
    req: web::Json<NewGameRequest>,
) -> Res {
    let conn = &mut pool.acquire().await.map_err(e500)?;
    let scene = match SceneRecord::load_from_key(conn, &req.scene_key).await {
        Ok(r) => match r.user(conn).await {
            Ok(user_id) => {
                if user.id == user_id {
                    r.load_scene(conn).await.map_err(e500)?
                } else {
                    return res_failure("Scene owned by a different user.");
                }
            }
            _ => return res_failure("Scene user not found."),
        },
        _ => return res_failure("Scene not found."),
    };

    let game_key = {
        let lock = games.read().await;
        loop {
            let game_key = generate_game_key().map_err(e500)?;
            if !lock.contains_key(&game_key) {
                break game_key;
            }
        }
    };

    if let Some(project) = scene.project {
        let pool = (*pool.into_inner()).clone();
        let server = launch_server(game_key.clone(), user.id, project, scene, pool);
        games.write().await.insert(game_key.clone(), server);
        let game_location = format!("/game/{game_key}");
        let resp = HttpResponse::Ok()
            .insert_header(("location", game_location.as_str()))
            .json(NewGameResponse {
                message: "Game launched successfully.".to_string(),
                success: true,
                url: game_location,
            });
        Ok(resp)
    } else {
        res_failure("Scene project unknown.")
    }
}

async fn join_game(
    req: HttpRequest,
    stream: web::Payload,
    games: Arc<Games>,
    user: User,
    game_key: &str,
) -> Res {
    let game = match games.read().await.get(game_key) {
        Some(handle) => handle.clone(),
        None => return Err(e500("Game not found.")),
    };

    let (resp, session, msg_stream) = actix_ws::handle(&req, stream)?;

    connect_client(user, game, session, msg_stream);

    Ok(resp)
}

async fn join(
    req: HttpRequest,
    stream: web::Payload,
    games: web::Data<Games>,
    user: User,
    path: web::Path<(String,)>,
) -> Res {
    let game_key = &path.into_inner().0;
    join_game(req, stream, games.into_inner(), user, game_key).await
}
