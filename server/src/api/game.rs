use std::sync::Arc;

use actix_web::web;
use sqlx::SqlitePool;
use tokio::sync::RwLock;

use super::{e500, res_failure, res_json, Res};
use crate::{
    crypto::random_hex_string,
    games::{generate_game_key, GameRef, GameServer, Games, GAME_KEY_LENGTH},
    models::{SceneRecord, User},
};

pub fn routes() -> actix_web::Scope {
    web::scope("/game")
        .route("/new", web::post().to(new))
        .route("/{game_key}", web::post().to(join))
        .route("/{game_key}/{client_key}", web::get().to(connect))
}

#[derive(serde_derive::Deserialize)]
struct NewGameRequest {
    scene_key: String,
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
        let games = games.read().await;
        loop {
            let game_key = random_hex_string(GAME_KEY_LENGTH).map_err(e500)?;
            if !games.contains_key(&game_key) {
                break game_key;
            }
        }
    };

    let games = &*games.into_inner();

    if let Some(project) = scene.project {
        let server = Arc::new(RwLock::new(GameServer::new(
            user.id,
            project,
            scene,
            (*pool.into_inner()).clone(),
            &game_key,
            games.clone(),
        )));
        GameServer::start(server.clone()).await;
        games.write().await.insert(game_key.clone(), server);

        join_game(games.clone(), game_key, user.id, user.username).await
    } else {
        res_failure("Scene project unknown.")
    }
}

#[derive(serde_derive::Serialize)]
struct JoinGameResponse {
    game_key: String,
    client_key: String,
    url: String,
    success: bool,
}

impl JoinGameResponse {
    fn new(game_key: String, client_key: String) -> Self {
        let url = format!("/game/{}/client/{}", &game_key, &client_key);

        Self {
            game_key,
            client_key,
            url,
            success: true,
        }
    }
}

async fn join_game(games: Games, game_key: String, user: i64, username: String) -> Res {
    let game = match games.read().await.get(&game_key) {
        Some(game_ref) => game_ref.clone(),
        None => return res_failure("Game not found."),
    };

    let client_key = generate_game_key().map_err(e500)?;

    game.write()
        .await
        .add_client(client_key.clone(), user, username);

    res_json(JoinGameResponse::new(game_key, client_key))
}

async fn join(games: web::Data<Games>, user: User, path: web::Data<(String,)>) -> Res {
    let games = (*games.into_inner()).clone();
    let game_key = path.into_inner().0.clone();
    join_game(games, game_key, user.id, user.username).await
}

async fn validate_game_and_client(
    game_key: &str,
    client_key: &str,
    games: &Games,
) -> Option<(GameRef, bool)> {
    if let Some(game_ref) = games.read().await.get(game_key) {
        return Some((
            game_ref.clone(),
            game_ref.read().await.has_client(client_key),
        ));
    }
    None
}

async fn connect(games: web::Data<Games>, path: web::Data<(String, String)>) -> Res {
    #[derive(serde_derive::Serialize)]
    struct Response {
        success: bool,
        game_valid: bool,
        client_valid: bool,
    }

    let (game_key, client_key) = &*path.into_inner();

    if let Some((_game, client_valid)) =
        validate_game_and_client(game_key, client_key, &games).await
    {
        res_json(&Response {
            success: true,
            game_valid: true,
            client_valid,
        })
    } else {
        res_json(&Response {
            success: true,
            game_valid: false,
            client_valid: false,
        })
    }
}
