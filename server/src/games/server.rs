use std::collections::HashMap;
use std::time;

use anyhow::anyhow;
use bincode::serialize;
use sqlx::{pool::PoolConnection, SqlitePool};
use tokio::sync::mpsc::UnboundedSender;
use warp::ws::Message;

use super::{client::Client, Game, GameRef};
use crate::{
    models::{Project, SceneRecord},
    scene::{
        comms::{ClientEvent, ClientMessage, ServerEvent},
        Scene,
    },
};

pub struct Server {
    alive: bool,
    pool: SqlitePool,
    last_action: time::SystemTime,
    clients: HashMap<String, Client>,
    owner: i64,
    game: Game,
}

impl Server {
    const SAVE_INTERVAL: time::Duration = time::Duration::from_secs(10);
    const INACTIVITY_TIMEOUT: time::Duration = time::Duration::from_secs(1800);

    pub fn new(owner: i64, project: i64, scene: Scene, pool: SqlitePool, key: &str) -> Self {
        Self {
            alive: true,
            pool,
            last_action: time::SystemTime::now(),
            clients: HashMap::new(),
            owner,
            game: super::Game::new(project, scene, owner, key),
        }
    }

    fn die(&mut self) {
        self.alive = false;
        // TODO properly kill server by closing web sockets &c
    }

    pub async fn start(server: GameRef) {
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
                    if let Err(e) = lock.save_scene().await {
                        eprintln!("Failed to save scene: {e}");
                    } else {
                        previous_action_time = lock.last_action;
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

            if player == self.owner {
                if let Ok(event) = self.scene_list().await {
                    self.send_to(event, &key);
                }
            }

            true
        } else {
            self.drop_client(&key);
            false
        }
    }

    fn get_client_mut(&mut self, key: &str) -> Option<&mut Client> {
        self.clients.get_mut(key)
    }

    fn is_owner(&self, key: &str) -> bool {
        if let Some(client) = self.clients.get(key) {
            client.user == self.owner
        } else {
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

    pub async fn handle_message(&mut self, message: ClientMessage, from: &str) {
        // Keep track of last activity. Even a ping will keep the server up.
        self.last_action = time::SystemTime::now();

        match message.event {
            ClientEvent::Ping => {
                self.send_approval(message.id, from);
            }
            ClientEvent::SceneChange(scene_key) => {
                if self.is_owner(from) {
                    if let Err(e) = self.load_scene(&scene_key).await {
                        eprintln!("Failed to load scene: {e}");
                        self.send_rejection(message.id, from);
                    } else {
                        self.send_approval(message.id, from);
                    }
                } else {
                    self.send_rejection(message.id, from);
                }
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

    async fn acquire_conn(&self) -> anyhow::Result<PoolConnection<sqlx::Sqlite>> {
        self.pool
            .acquire()
            .await
            .map_err(|e| anyhow!("Failed to acquire connection: {e}"))
    }

    async fn replace_scene(&mut self, scene: Scene) {
        self.save_scene().await.ok(); // If this fails, so be it

        // Replace the local scene with the new one
        self.game.replace_scene(scene.clone(), self.owner);

        // Send the new scene and perms to all clients
        let keys: Vec<(String, i64)> = self.clients.iter().map(|(k, c)| (k.to_owned(), c.user)).collect();
        for (client_key, user) in keys {
            self.game.add_player(user);

            let scene_change = ServerEvent::SceneChange(self.game.client_scene());
            let perms_change = ServerEvent::PermsChange(self.game.client_perms());

            self.send_to(scene_change, &client_key);
            self.send_to(perms_change, &client_key);
        }
    }

    async fn load_scene(&mut self, scene_key: &str) -> anyhow::Result<()> {
        let conn = &mut self.acquire_conn().await?;
        let record = SceneRecord::load_from_key(conn, scene_key).await?;
        let scene = record.load_scene(conn).await?;

        self.replace_scene(scene).await;

        Ok(())
    }

    async fn save_scene(&self) -> anyhow::Result<()> {
        let conn = &mut self.acquire_conn().await?;
        if let Some(id) = self.game.project_id() {
            let project = Project::load(conn, id).await?;
            project
                .update_scene(conn, self.game.server_scene())
                .await
                .map(|_| ())
        } else {
            Err(anyhow!("Scene has no project."))
        }
    }

    async fn scene_list(&self) -> anyhow::Result<ServerEvent> {
        let conn = &mut self.acquire_conn().await?;
        if let Some(id) = self.game.project_id() {
            let project = Project::load(conn, id).await?;
            let current_scene_id = self.game.scene_id();
            let mut current = String::from("");
            let scenes = project
                .list_scenes(conn)
                .await?
                .into_iter()
                .map(|scene| {
                    if current_scene_id == Some(scene.id) {
                        current = scene.scene_key.clone();
                    }

                    (scene.scene_key, scene.title)
                })
                .collect();
            Ok(ServerEvent::SceneList(scenes, current))
        } else {
            Err(anyhow!("Failed to find project."))
        }
    }
}
