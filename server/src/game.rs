use std::collections::HashMap;
use std::sync::Arc;

use futures::{FutureExt, StreamExt};
use tokio::sync::{
    mpsc::{self, UnboundedSender},
    RwLock,
};
use warp::ws::{Message, WebSocket};

use scene::Scene;

use crate::crypto::random_hex_string;

struct Client {
    user: i64,
    sender: Option<mpsc::UnboundedSender<Result<Message, warp::Error>>>,
}

pub struct Game {
    clients: HashMap<String, Client>,
    owner: i64,
    scene: Scene,
}

impl Game {
    fn new(owner: i64, scene: Scene) -> Self {
        Game {
            clients: HashMap::new(),
            owner,
            scene,
        }
    }

    pub fn new_ref(owner: i64, scene: Scene) -> GameRef {
        Arc::new(RwLock::new(Self::new(owner, scene)))
    }

    pub fn add_client(&mut self, key: String, user: i64) {
        self.clients.insert(key, Client { user, sender: None });
    }

    pub fn has_client(&self, key: &str) -> bool {
        self.clients.contains_key(key)
    }

    fn get_client_mut(&mut self, key: &str) -> Option<&mut Client> {
        self.clients.get_mut(key)
    }

    fn drop_client(&mut self, key: String) {
        self.clients.remove(&key);
    }

    fn connect_client(
        &mut self,
        key: String,
        sender: UnboundedSender<Result<Message, warp::Error>>,
    ) -> bool {
        if let Some(client) = self.get_client_mut(&key) {
            client.sender = Some(sender);
            true
        } else {
            self.drop_client(key);
            false
        }
    }

    fn event(&self, key: &str) {
        println!("Event from session {}", key);
    }
}

pub type GameRef = Arc<RwLock<Game>>;
pub type Games = Arc<RwLock<HashMap<String, GameRef>>>;

pub const GAME_KEY_LENGTH: usize = 6;

pub async fn client_connection(ws: WebSocket, key: String, game: GameRef) {
    let (client_ws_send, mut client_ws_recv) = ws.split();
    let (client_send, client_recv) = mpsc::unbounded_channel();
    let client_recv = tokio_stream::wrappers::UnboundedReceiverStream::new(client_recv);
    tokio::task::spawn(client_recv.forward(client_ws_send).map(|result| {
        if let Err(e) = result {
            eprintln!("error sending websocket msg: {}", e);
        }
    }));

    if !game.write().await.connect_client(key.clone(), client_send) {
        return;
    }

    while let Some(result) = client_ws_recv.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("error receiving ws message: {}", e);
                break;
            }
        };

        game.read().await.event(&key);
    }

    game.write().await.drop_client(key);
}

pub fn generate_game_key() -> anyhow::Result<String> {
    random_hex_string(GAME_KEY_LENGTH)
}
