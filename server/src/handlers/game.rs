use std::convert::Infallible;

use warp::Filter;

use crate::game::Games;

pub fn with_games(games: Games) -> impl Filter<Extract = (Games,), Error = Infallible> + Clone {
    warp::any().map(move || games.clone())
}

mod new {
    use std::convert::Infallible;

    use serde_derive::Serialize;
    use sqlx::SqlitePool;
    use warp::Filter;

    use crate::game;
    use crate::handlers::crypto::random_hex_string;
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
