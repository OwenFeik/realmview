use crate::{
    crypto::random_hex_string,
    models::User,
    utils::{warning, Res},
};

mod client;
mod game;
mod server;

pub use server::GameHandle;
use uuid::Uuid;

pub const GAME_KEY_LENGTH: usize = 10;

pub fn generate_game_key() -> Res<String> {
    random_hex_string(GAME_KEY_LENGTH)
}

pub fn launch_server(
    key: String,
    owner: Uuid,
    project: Uuid,
    scene: scene::Scene,
    pool: sqlx::SqlitePool,
) -> GameHandle {
    server::launch(key, owner, project, scene, pool)
}

pub fn connect_client(
    user: User,
    server: GameHandle,
    session: actix_ws::Session,
    stream: actix_ws::MessageStream,
) {
    client::connect_game_client(user, server, session, stream);
}

pub async fn close_ws(session: actix_ws::Session) {
    const CLOSE_REASON: &str = "gameover";

    if let Err(e) = session
        .close(Some(actix_ws::CloseReason {
            code: actix_ws::CloseCode::Normal,
            description: Some(CLOSE_REASON.to_string()),
        }))
        .await
    {
        warning(format!("Error when closing WS: {e}"));
    }
}
