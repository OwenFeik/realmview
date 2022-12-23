use std::collections::HashMap;
use std::time;

use bincode::serialize;
use sqlx::{SqliteConnection, SqlitePool};
use tokio::sync::mpsc::UnboundedSender;
use warp::ws::Message;

use super::{client::Client, Game, GameRef};
use crate::{
    models::Project,
    scene::{
        comms::{ClientEvent, ClientMessage, ServerEvent},
        Scene,
    },
};

pub struct Server {
    alive: bool,
    last_action: time::SystemTime,
    clients: HashMap<String, Client>,
    owner: i64,
    game: Game,
}

impl Server {
    const SAVE_INTERVAL: time::Duration = time::Duration::from_secs(10);
    const INACTIVITY_TIMEOUT: time::Duration = time::Duration::from_secs(1800);

    pub fn new(owner: i64, scene: Scene, key: &str) -> Self {
        Self {
            alive: true,
            last_action: time::SystemTime::now(),
            clients: HashMap::new(),
            owner,
            game: super::Game::new(scene, owner, key),
        }
    }

    fn die(&mut self) {
        self.alive = false;
        // TODO properly kill server by closing web sockets &c
    }

    pub async fn start(server: GameRef, pool: SqlitePool) {
        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(Self::SAVE_INTERVAL);

            let mut previous_action_time = server.read().await.last_action;
            while server.read().await.alive {
                // Wait for SAVE_INTERVAL
                interval.tick().await;

                // Grab a read lock on the server
                let lock = server.read().await;

                // If something's changed in the scene, save it
                if lock.last_action > previous_action_time {
                    if let Ok(mut conn) = pool.acquire().await {
                        if let Err(e) = lock.save(&mut conn).await {
                            eprintln!("Failed to save scene: {e}");
                        }
                        previous_action_time = lock.last_action;
                    } else {
                        dbg!("Failed to acquire database connection.");
                    }
                }

                // If the server has timed out, close it down
                if let Ok(duration) = time::SystemTime::now().duration_since(lock.last_action) {
                    if duration > Self::INACTIVITY_TIMEOUT {
                        println!("Closing {} due to inactivity.", &lock.game.key);

                        // Drop read lock so we can get a write lock
                        drop(lock);
                        server.write().await.die();
                    }
                }
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
        // Keep track of last activity. Even a ping will keep the server up.
        self.last_action = time::SystemTime::now();

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

    async fn save(&self, conn: &mut SqliteConnection) -> anyhow::Result<()> {
        if let Some(id) = self.game.project_id() {
            let project = Project::load(conn, id).await?;
            project
                .update_scene(conn, self.game.server_scene())
                .await
                .map(|_| ())
        } else {
            Err(anyhow::anyhow!("Scene has no project."))
        }
    }
}
