use std::collections::HashMap;

use bincode::serialize;
use tokio::sync::mpsc::UnboundedSender;
use warp::ws::Message;

use super::{client::Client, Game, GameRef};
use crate::scene::{
    comms::{ClientEvent, ClientMessage, ServerEvent},
    Scene,
};

pub struct Server {
    clients: HashMap<String, Client>,
    owner: i64,
    game: Game,
}

impl Server {
    const SAVE_INTERVAL_SECONDS: u64 = 10;

    pub fn new(owner: i64, scene: Scene, key: &str) -> Self {
        Self {
            clients: HashMap::new(),
            owner,
            game: super::Game::new(scene, owner, key),
        }
    }

    pub async fn start(server: GameRef) {
        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(Self::SAVE_INTERVAL_SECONDS));
            
            loop {
                interval.tick().await;
                server.read().await.save().await;
            }
        });
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
            let player = client.user;

            if let Some(event) = self.game.add_player(player) {
                self.broadcast_event(ServerEvent::PermsUpdate(event), Some(&key));
            }

            self.send_to(ServerEvent::UserId(player), &key);

            let scene = self.game.client_scene();
            self.send_to(ServerEvent::SceneChange(scene), &key);
            let perms = self.game.client_perms();
            self.send_to(ServerEvent::PermsChange(perms), &key);
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

    pub async fn handle_message(&mut self, message: ClientMessage, from: &str) {
        match message.event {
            ClientEvent::Ping => {
                self.send_approval(message.id, from);
            }
            ClientEvent::SceneUpdate(event) => {
                if let Some(client) = self.clients.get(from) {
                    let (ok, perms_events) = self.game.handle_event(client.user, event.clone());

                    if ok {
                        self.send_approval(message.id, from);
                        self.broadcast_event(ServerEvent::SceneUpdate(event), Some(from));
                    } else {
                        println!("Rejected event: {event:?}");
                        self.send_rejection(message.id, from);
                    }

                    if let Some(events) = perms_events {
                        for event in events {
                            self.broadcast_event(ServerEvent::PermsUpdate(event), None);
                        }
                    }
                }
            }
        };
    }

    async fn save(&self) {
        println!("TODO: Implement saving.");
    }
}
