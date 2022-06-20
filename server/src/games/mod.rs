use std::collections::HashMap;
use std::sync::Arc;

use bincode::deserialize;
use futures::{SinkExt, StreamExt, TryFutureExt};
use tokio::sync::{mpsc::unbounded_channel, RwLock};
use warp::ws::WebSocket;

use crate::crypto::random_hex_string;

mod client;
mod game;
mod perms;
mod server;

pub use game::Game;
pub use server::Server as GameServer;

pub type GameRef = Arc<RwLock<GameServer>>;
pub type Games = Arc<RwLock<HashMap<String, GameRef>>>;

pub const GAME_KEY_LENGTH: usize = 6;

pub async fn client_connection(ws: WebSocket, key: String, game: GameRef) {
    let (mut client_ws_send, mut client_ws_recv) = ws.split();
    let (client_send, client_recv) = unbounded_channel();
    let mut client_recv = tokio_stream::wrappers::UnboundedReceiverStream::new(client_recv);
    tokio::task::spawn(async move {
        while let Some(msg) = client_recv.next().await {
            client_ws_send
                .send(msg)
                .unwrap_or_else(|e| eprintln!("Error sending websocket msg: {}", e))
                .await;
        }
    });

    if !game
        .write()
        .await
        .connect_client(key.clone(), client_send)
        .await
    {
        return;
    }

    while let Some(result) = client_ws_recv.next().await {
        match result {
            Ok(msg) => match deserialize(msg.as_bytes()) {
                Ok(message) => game.read().await.handle_message(message, &key).await,
                Err(e) => eprintln!("Error parsing ws message: {}", e),
            },
            Err(e) => {
                eprintln!("Error receiving ws message: {}", e);
                break;
            }
        };
    }

    game.write().await.drop_client(&key);
    println!("Dropped client {key}");
}

pub fn generate_game_key() -> anyhow::Result<String> {
    random_hex_string(GAME_KEY_LENGTH)
}
