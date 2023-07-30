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

    use super::Games;
    use crate::games::GameRef;

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

    async fn connect_to_game(
        game_key: String,
        client_key: String,
        ws: warp::ws::Ws,
        games: Games,
    ) -> Result<impl warp::Reply, warp::Rejection> {
        if let Some((game, true)) = validate_game_and_client(&game_key, &client_key, &games).await {
            Ok(ws.on_upgrade(move |sock| crate::games::client_connection(sock, client_key, game)))
        } else {
            Err(warp::reject())
        }
    }

    async fn test_game_valid(
        game_key: String,
        client_key: String,
        games: Games,
    ) -> Result<impl warp::Reply, warp::Rejection> {
        #[derive(serde_derive::Serialize)]
        struct Response {
            success: bool,
            game_valid: bool,
            client_valid: bool,
        }

        let resp = if let Some((_game, client_valid)) =
            validate_game_and_client(&game_key, &client_key, &games).await
        {
            warp::reply::json(&Response {
                success: true,
                game_valid: true,
                client_valid,
            })
        } else {
            warp::reply::json(&Response {
                success: true,
                game_valid: false,
                client_valid: false,
            })
        };

        Ok(resp)
    }

    pub fn filter(
        games: super::Games,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        (warp::path!("game" / String / String)
            .and(warp::ws())
            .and(super::with_games(games.clone()))
            .and_then(connect_to_game))
        .or(warp::path!("game" / String / String)
            .and(super::with_games(games))
            .and_then(test_game_valid))
    }
}

mod join {
    use serde_derive::Serialize;
    use warp::http::StatusCode;
    use warp::Filter;

    use crate::games::{generate_game_key, GameRef, Games};
    use crate::models::User;
    use crate::warp_handlers::response::{as_result, Binary, ResultReply};

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
            .and(crate::warp_handlers::with_db(pool))
            .and(crate::warp_handlers::with_session())
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
    use crate::models::{SceneRecord, User};
    use crate::warp_handlers::{json_body, response::Binary, with_db, with_session};

    async fn new_game(
        pool: SqlitePool,
        skey: String,
        games: games::Games,
        req: NewGameRequest,
    ) -> Result<impl warp::Reply, Infallible> {
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
