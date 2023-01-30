use std::convert::Infallible;

use warp::Filter;

use crate::games::Games;

pub fn routes(
    pool: sqlx::SqlitePool,
    games: Games,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    new::filter(pool.clone(), games.clone())
        .or(join::filter(pool, games.clone()))
        .or(connect::filter(games))
}

fn with_games(games: Games) -> impl Filter<Extract = (Games,), Error = Infallible> + Clone {
    warp::any().map(move || games.clone())
}

mod connect {
    use warp::Filter;

    async fn connect_to_game(
        game_key: String,
        client_key: String,
        ws: warp::ws::Ws,
        games: super::Games,
    ) -> Result<impl warp::Reply, warp::Rejection> {
        let game = match games.read().await.get(&game_key) {
            Some(game_ref) => {
                if !game_ref.read().await.has_client(&client_key) {
                    return Err(warp::reject());
                }

                game_ref.clone()
            }
            None => return Err(warp::reject()),
        };

        Ok(ws.on_upgrade(move |sock| crate::games::client_connection(sock, client_key, game)))
    }

    pub fn filter(
        games: super::Games,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("game" / String / String)
            .and(warp::ws())
            .and(super::with_games(games))
            .and_then(connect_to_game)
    }
}

mod join {
    use serde_derive::Serialize;
    use warp::http::StatusCode;
    use warp::Filter;

    use crate::games::{generate_game_key, GameRef, Games};
    use crate::handlers::response::{as_result, Binary, ResultReply};
    use crate::models::User;

    #[derive(Serialize)]
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

    async fn add_client(game: GameRef, user_id: i64, user_name: String) -> anyhow::Result<String> {
        let client_key = generate_game_key()?;

        game.write()
            .await
            .add_client(client_key.clone(), user_id, user_name);

        Ok(client_key)
    }

    pub async fn join_game(
        games: Games,
        game_key: String,
        user_id: i64,
        user_name: String,
    ) -> ResultReply {
        let game = match games.read().await.get(&game_key) {
            Some(game_ref) => game_ref.clone(),
            None => return Binary::result_error("Game not found."),
        };

        match add_client(game, user_id, user_name).await {
            Ok(client_key) => {
                as_result(&JoinGameResponse::new(game_key, client_key), StatusCode::OK)
            }
            Err(_) => Binary::result_error("Cryptography error."),
        }
    }

    async fn join_game_handler(
        game_key: String,
        games: Games,
        pool: sqlx::SqlitePool,
        skey: String,
    ) -> Result<impl warp::Reply, super::Infallible> {
        let user = match User::get_by_session(&pool, &skey).await {
            Ok(Some(u)) => u,
            _ => return Binary::result_failure("Bad session."),
        };

        join_game(games, game_key, user.id, user.username).await
    }

    pub fn filter(
        pool: sqlx::SqlitePool,
        games: Games,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("game" / String)
            .and(warp::post())
            .and(super::with_games(games))
            .and(crate::handlers::with_db(pool))
            .and(crate::handlers::with_session())
            .and_then(join_game_handler)
    }
}

mod new {
    use std::convert::Infallible;
    use std::sync::Arc;

    use serde_derive::Deserialize;
    use sqlx::SqlitePool;
    use tokio::sync::RwLock;
    use warp::Filter;

    use super::with_games;
    use crate::crypto::random_hex_string;
    use crate::games;
    use crate::handlers::{json_body, response::Binary, with_db, with_session};
    use crate::models::{SceneRecord, User};

    #[derive(Deserialize)]
    struct NewGameRequest {
        scene_key: String,
    }

    async fn new_game(
        pool: SqlitePool,
        skey: String,
        games: games::Games,
        req: NewGameRequest,
    ) -> Result<impl warp::Reply, Infallible> {
        let user = match User::get_by_session(&pool, &skey).await {
            Ok(Some(u)) => u,
            _ => return Binary::result_failure("Bad session."),
        };

        let conn = &mut match pool.acquire().await {
            Ok(c) => c,
            _ => return Binary::result_error("Database error."),
        };

        let scene = match SceneRecord::load_from_key(conn, &req.scene_key).await {
            Ok(r) => match r.user(conn).await {
                Ok(user_id) => {
                    if user.id == user_id {
                        match r.load_scene(conn).await {
                            Ok(s) => s,
                            _ => return Binary::result_error("Failed to load scene."),
                        }
                    } else {
                        return Binary::result_failure("Scene owned by a different user.");
                    }
                }
                _ => return Binary::result_failure("Scene user not found."),
            },
            _ => return Binary::result_failure("Scene not found."),
        };

        let game_key = {
            let games = games.read().await;
            loop {
                if let Ok(game_key) = random_hex_string(games::GAME_KEY_LENGTH) {
                    if !games.contains_key(&game_key) {
                        break game_key;
                    }
                } else {
                    return Binary::result_error("Crypto error.");
                }
            }
        };

        if let Some(project) = scene.project {
            let server = Arc::new(RwLock::new(games::GameServer::new(
                user.id,
                project,
                scene,
                pool.clone(),
                &game_key,
                games.clone(),
            )));
            games::GameServer::start(server.clone()).await;
            games.write().await.insert(game_key.clone(), server);

            super::join::join_game(games, game_key, user.id, user.username).await
        } else {
            Binary::result_failure("Scene project unknown.")
        }
    }

    pub fn filter(
        pool: SqlitePool,
        games: crate::Games,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path("game")
            .and(warp::path("new"))
            .and(warp::post())
            .and(with_db(pool))
            .and(with_session())
            .and(with_games(games))
            .and(json_body())
            .and_then(new_game)
    }
}
