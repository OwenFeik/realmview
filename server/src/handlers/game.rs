use std::convert::Infallible;

use warp::Filter;

use crate::game::Games;

pub fn with_games(games: Games) -> impl Filter<Extract = (Games,), Error = Infallible> + Clone {
    warp::any().map(move || games.clone())
}

mod connect {
    use warp::Filter;

    use crate::handlers::response::Binary;

    async fn connect_to_game(
        game_key: String,
        client_key: String,
        ws: warp::ws::Ws,
        games: super::Games,
    ) -> Result<impl warp::Reply, super::Infallible> {
        let game = match games.read().await.get(&game_key) {
            Some(game_ref) => {
                if game_ref.read().await.has_client(&client_key) {
                    return Binary::result_failure("Client key invalid.");
                }

                game_ref.clone()
            }
            None => return Binary::result_failure("Game key invalid."),
        };

        ws.on_upgrade(move |sock| crate::game::client_connection(sock, client_key, game));
        Binary::result_success("Connected to game.")
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

    use crate::game::{generate_game_key, GameRef, Games};
    use crate::handlers::response::{as_result, Binary};
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
            let url = format!("game/{}/{}", &game_key, &client_key);

            Self {
                game_key,
                client_key,
                url,
                success: true,
            }
        }
    }

    async fn add_client(game: GameRef, user_id: i64) -> anyhow::Result<String> {
        let client_key = generate_game_key()?;

        game.write().await.add_client(client_key.clone(), user_id);

        Ok(client_key)
    }

    async fn join_game(
        game_key: String,
        games: Games,
        pool: sqlx::SqlitePool,
        skey: String,
    ) -> Result<impl warp::Reply, super::Infallible> {
        let game = match games.read().await.get(&game_key) {
            Some(game_ref) => game_ref.clone(),
            None => return Binary::result_error("Game key invalid."),
        };

        let user = match User::get_by_session(&pool, &skey).await {
            Ok(Some(u)) => u,
            _ => return Binary::result_failure("Bad session."),
        };

        match add_client(game, user.id).await {
            Ok(client_key) => {
                as_result(&JoinGameResponse::new(game_key, client_key), StatusCode::OK)
            }
            Err(_) => Binary::result_error("Crypography error."),
        }
    }

    pub fn filter(
        pool: sqlx::SqlitePool,
        games: Games,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("game" / String)
            .and(super::with_games(games))
            .and(crate::handlers::with_db(pool))
            .and(crate::handlers::with_session())
            .and_then(join_game)
    }
}

mod new {
    use std::convert::Infallible;

    use serde_derive::Serialize;
    use sqlx::SqlitePool;
    use warp::Filter;

    use crate::crypto::random_hex_string;
    use crate::game;
    use crate::handlers::{
        response::{as_result, Binary},
        with_db, with_session,
    };
    use crate::models::User;

    use super::with_games;

    #[derive(Serialize)]
    struct NewGameResponse {
        game_key: String,
        success: bool,
    }

    impl NewGameResponse {
        fn new(game_key: String) -> Self {
            NewGameResponse {
                game_key,
                success: true,
            }
        }
    }

    async fn new_game(
        pool: SqlitePool,
        skey: String,
        games: game::Games,
    ) -> Result<impl warp::Reply, Infallible> {
        let user = match User::get_by_session(&pool, &skey).await {
            Ok(Some(u)) => u,
            _ => return Binary::result_failure("Bad session."),
        };

        let mut games = games.write().await;
        let game_key = loop {
            if let Ok(game_key) = random_hex_string(game::GAME_KEY_LENGTH) {
                if !games.contains_key(&game_key) {
                    break game_key;
                }
            } else {
                return Binary::result_error("Crypto error.");
            }
        };

        games.insert(
            game_key.clone(),
            game::Game::new_ref(user.id, scene::Scene::new()),
        );
        as_result(&NewGameResponse::new(game_key), warp::http::StatusCode::OK)
    }

    pub fn filter(
        pool: SqlitePool,
        games: crate::Games,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path("game/new")
            .and(with_db(pool))
            .and(with_session())
            .and(with_games(games))
            .and_then(new_game)
    }
}
