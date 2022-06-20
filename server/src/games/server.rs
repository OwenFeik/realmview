use std::collections::HashMap;

use bincode::serialize;
use tokio::sync::{mpsc::UnboundedSender, RwLock};
use warp::ws::Message;

use scene::comms::{ClientEvent, ClientMessage, ServerEvent};

use super::client::Client;
use super::game::Game;

pub struct Server {
    clients: HashMap<String, Client>,
    owner: i64,
    game: RwLock<Game>,
}

impl Server {
    pub fn new(owner: i64, game: Game) -> Self {
        Server {
            clients: HashMap::new(),
            owner,
            game: RwLock::new(game),
        }
    }

    pub fn add_client(&mut self, key: String, user: i64) {
        self.clients.insert(key, Client::new(user));
    }

    pub fn has_client(&self, key: &str) -> bool {
        self.clients.contains_key(key)
    }

    pub fn drop_client(&mut self, key: &str) {
        self.clients.remove(key);
    }

    pub async fn connect_client(&mut self, key: String, sender: UnboundedSender<Message>) -> bool {
        if let Some(client) = self.get_client_mut(&key) {
            client.set_sender(sender);
            self.send_to(
                ServerEvent::SceneChange(self.game.write().await.client_scene()),
                &key,
            );
            true
        } else {
            self.drop_client(&key);
            false
        }
    }

    fn get_client_mut(&mut self, key: &str) -> Option<&mut Client> {
        self.clients.get_mut(key)
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

    pub async fn handle_message(&self, message: ClientMessage, from: &str) {
        match message.event {
            ClientEvent::Ping => {
                self.send_approval(message.id, from);
            }
            ClientEvent::SceneChange(event) => {
                if let Some(client) = self.clients.get(from) {
                    if self
                        .game
                        .write()
                        .await
                        .handle_event(client.user, event.clone())
                    {
                        self.send_approval(message.id, from);
                        self.broadcast_event(ServerEvent::SceneUpdate(event), Some(from));
                    } else {
                        self.send_rejection(message.id, from);
                    }
                }
            }
        };
    }
}
