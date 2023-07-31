use std::collections::HashMap;
use std::sync::Arc;

use bincode::deserialize;
use futures::StreamExt;
use tokio::sync::RwLock;
use warp::ws::WebSocket;

use crate::{
    crypto::random_hex_string,
    utils::{debug, error},
};

mod client;
mod game;
mod server;
mod sock;

pub use game::Game;
pub use server::Server as GameServer;

pub type GameRef = Arc<RwLock<GameServer<sock::ActixWs>>>;
pub type Games = Arc<RwLock<HashMap<String, GameRef>>>;

pub const GAME_KEY_LENGTH: usize = 6;

pub async fn client_connection(ws: WebSocket, key: String, game: GameRef) {
    let (client_ws_send, mut client_ws_recv) = ws.split();
    let Ok(conn_id) = game
        .write()
        .await
        .connect_client(key.clone(), client_ws_send)
        .await
    else {
        return;
    };

    while let Some(result) = client_ws_recv.next().await {
        match result {
            Ok(msg) => match deserialize(msg.as_bytes()) {
                Ok(message) => {
                    if !game.write().await.handle_message(message, &key).await {
                        break;
                    }
                }
                Err(e) => match *e {
                    bincode::ErrorKind::Io(err)
                        if err.kind() == std::io::ErrorKind::UnexpectedEof =>
                    {
                        // EOF error is returned when the websocket closes.
                        debug("Websocket receiver closed")
                    }
                    _ => error(format!("Error parsing ws message: {e}")),
                },
            },
            Err(_) => break,
        };
    }

    game.write().await.disconnect_client(&key, conn_id);
}

pub fn generate_game_key() -> anyhow::Result<String> {
    random_hex_string(GAME_KEY_LENGTH)
}
