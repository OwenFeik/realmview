use std::collections::HashMap;
use std::sync::Arc;

use futures::{FutureExt, StreamExt};
use tokio::sync::{mpsc, RwLock};
use warp::ws::{Message, WebSocket};

use scene::Scene;

struct Client {
    user: i64,
    sender: Option<mpsc::UnboundedSender<Result<Message, warp::Error>>>,
}

pub struct Game {
    owner: i64,
    scene: Scene,
}

impl Game {
    fn new(owner: i64, scene: Scene) -> Self {
        Game { owner, scene }
    }

    pub fn new_ref(owner: i64, scene: Scene) -> GameRef {
        Arc::new(RwLock::new(Self::new(owner, scene)))
    }

    fn event(&self, user: i64) {
        println!("Event from user {}", user);
    }
}

pub type GameRef = Arc<RwLock<Game>>;
pub type Games = Arc<RwLock<HashMap<String, GameRef>>>;

pub const GAME_KEY_LENGTH: usize = 6;

async fn client_connection(ws: WebSocket, mut client: Client, game: Arc<RwLock<Game>>) {
    let (client_ws_send, mut client_ws_recv) = ws.split();
    let (client_send, client_recv) = mpsc::unbounded_channel();
    let client_recv = tokio_stream::wrappers::UnboundedReceiverStream::new(client_recv);
    tokio::task::spawn(client_recv.forward(client_ws_send).map(|result| {
        if let Err(e) = result {
            eprintln!("error sending websocket msg: {}", e);
        }
    }));

    client.sender = Some(client_send);

    while let Some(result) = client_ws_recv.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("error receiving ws message: {}", e);
                break;
            }
        };

        game.read().await.event(client.user);
    }
}
