use crate::{models::User, utils::warning};

mod client;
mod game;
mod server;

pub use game::GameKey;
pub use server::launch as launch_server;
pub use server::GameHandle;

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
