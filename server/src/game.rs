use std::collections::HashMap;
use std::sync::Arc;

use bincode::{deserialize, serialize};
use futures::{SinkExt, StreamExt, TryFutureExt};
use tokio::sync::{
    mpsc::{self, UnboundedSender},
    RwLock,
};
use warp::ws::{Message, WebSocket};

use scene::{
    comms::{ClientEvent, ServerEvent},
    Scene,
};

use crate::crypto::random_hex_string;

struct Client {
    user: i64,
    sender: Option<mpsc::UnboundedSender<Message>>,
}

impl Client {
    fn send(&self, message: Message) {
        if let Some(sender) = &self.sender {
            sender.send(message).ok();
        }
    }
}

pub struct Game {
    clients: HashMap<String, Client>,
    owner: i64,
    scene: RwLock<Scene>,
}

impl Game {
    fn new(owner: i64, scene: Scene) -> Self {
        Game {
            clients: HashMap::new(),
            owner,
            scene: RwLock::new(scene),
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

    fn connect_client(&mut self, key: String, sender: UnboundedSender<Message>) -> bool {
        if let Some(client) = self.get_client_mut(&key) {
            client.sender = Some(sender);
            true
        } else {
            self.drop_client(key);
            false
        }
    }

    fn broadcast_event(&self, event: ServerEvent, exclude: Option<&str>) {
        let data = match serialize(&event) {
            Ok(e) => e,
            Err(_) => return,
        };

        let clients = self.clients.iter();
        if let Some(key) = exclude {
            clients.for_each(|(k, c)| {
                if *key != *k {
                    c.send(Message::binary(data.clone()));
                }
            });
        } else {
            clients.for_each(|(_, c)| c.send(Message::binary(data.clone())));
        }
    }

    fn send_to(&self, event: ServerEvent, client_key: &str) {
        if let Some(client) = self.clients.get(client_key) {
            if let Ok(data) = serialize(&event) {
                client.send(Message::binary(data))
            }
        }
    }

    fn send_approval(&self, event_id: i64, client_key: &str) {
        self.send_to(ServerEvent::Approval(event_id), client_key);
    }

    fn send_rejection(&self, event_id: i64, client_key: &str) {
        self.send_to(ServerEvent::Rejection(event_id), client_key);
    }

    async fn handle_event(&self, event: ClientEvent, from: &str) {
        if self
            .scene
            .write()
            .await
            .apply_event(&event.scene_event, false)
        {
            self.send_approval(event.id, from);
            self.broadcast_event(
                ServerEvent::SceneChange(event.scene_event, None),
                Some(from),
            );
        } else {
            self.send_rejection(event.id, from);
        }
    }
}

pub type GameRef = Arc<RwLock<Game>>;
pub type Games = Arc<RwLock<HashMap<String, GameRef>>>;

pub const GAME_KEY_LENGTH: usize = 6;

pub async fn client_connection(ws: WebSocket, key: String, game: GameRef) {
    let (mut client_ws_send, mut client_ws_recv) = ws.split();
    let (client_send, client_recv) = mpsc::unbounded_channel();
    let mut client_recv = tokio_stream::wrappers::UnboundedReceiverStream::new(client_recv);
    tokio::task::spawn(async move {
        while let Some(msg) = client_recv.next().await {
            client_ws_send
                .send(msg)
                .unwrap_or_else(|e| eprintln!("error sending websocket msg: {}", e))
                .await;
        }
    });

    if !game.write().await.connect_client(key.clone(), client_send) {
        return;
    }

    while let Some(result) = client_ws_recv.next().await {
        match result {
            Ok(msg) => match deserialize(msg.as_bytes()) {
                Ok(event) => game.read().await.handle_event(event, &key).await,
                Err(e) => eprintln!("error parsing ws message: {}", e),
            },
            Err(e) => {
                eprintln!("error receiving ws message: {}", e);
                break;
            }
        };
    }

    game.write().await.drop_client(key);
}

pub fn generate_game_key() -> anyhow::Result<String> {
    random_hex_string(GAME_KEY_LENGTH)
}
