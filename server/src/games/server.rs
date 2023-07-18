use std::collections::HashMap;
use std::time;

use anyhow::anyhow;
use bincode::serialize;
use sqlx::{pool::PoolConnection, SqlitePool};
use tokio::sync::mpsc::UnboundedSender;
use warp::ws::Message;

use super::{client::Client, Game, GameRef, Games};
use crate::{
    models::{Project, SceneRecord},
    scene::{
        comms::{ClientEvent, ClientMessage, ServerEvent},
        Scene,
    },
    utils::{log, timestamp_us, LogLevel},
};

pub struct Server {
    alive: bool,
    pool: SqlitePool,
    last_action: time::SystemTime,
    clients: HashMap<String, Client>,
    owner: i64,
    game: Game,
    games: Games,
}

impl Server {
    const SAVE_INTERVAL: time::Duration = time::Duration::from_secs(10);
    const INACTIVITY_TIMEOUT: time::Duration = time::Duration::from_secs(30);

    pub fn new(
        owner: i64,
        project: i64,
        scene: Scene,
        pool: SqlitePool,
        key: &str,
        games: Games,
    ) -> Self {
        Self {
            alive: true,
            pool,
            last_action: time::SystemTime::now(),
            clients: HashMap::new(),
            owner,
            game: super::Game::new(project, scene, owner, key),
            games,
        }
    }

    async fn die(&mut self) {
        self.alive = false;

        for client in self.clients.values_mut() {
            client.clear_sender();
        }

        self.save_scene().await.ok();

        // Delete this game.
        self.games.write().await.remove(&self.game.key);

        self.log(LogLevel::Debug, "Closed server");
    }

    pub async fn start(server: GameRef) {
        server.read().await.log(LogLevel::Info, "Opened server");
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
                    match lock.save_scene().await {
                        Ok(duration) => {
                            previous_action_time = lock.last_action;
                            lock.log(
                                LogLevel::Debug,
                                format!("Saved scene. Save duration: {duration}us"),
                            )
                        }
                        Err(e) => lock.log(LogLevel::Error, format!("Failed to save scene: {e}")),
                    }
                }

                // If the server has timed out, close it down
                if let Ok(duration) = time::SystemTime::now().duration_since(lock.last_action) {
                    if duration > Self::INACTIVITY_TIMEOUT {
                        lock.log(LogLevel::Debug, "Closing due to inactivity");

                        // Drop read lock so we can get a write lock
                        drop(lock);
                        server.write().await.die().await;
                    }
                }
            }
        });
    }

    pub fn add_client(&mut self, key: String, user: i64, username: String) {
        self.log(LogLevel::Debug, format!("New client ({key})"));
        self.clients.insert(key, Client::new(user, username));
    }

    pub fn has_client(&self, key: &str) -> bool {
        self.clients.contains_key(key)
    }

    pub fn drop_client(&mut self, key: &str) {
        if let Some(client) = self.clients.get_mut(key) {
            client.clear_sender();
            self.log(LogLevel::Debug, format!("Client disconnected ({key})"));
        }
    }

    pub async fn connect_client(&mut self, key: String, sender: UnboundedSender<Message>) -> bool {
        let (player, name) = if let Some(client) = self.get_client_mut(&key) {
            client.set_sender(sender);
            (client.user, client.username.clone())
        } else {
            self.drop_client(&key);
            return false;
        };

        let (perms, scene, layer) = self.game.add_player(player, &name);

        if let Some(event) = perms {
            self.broadcast_event(ServerEvent::PermsUpdate(event), Some(&key));
        }

        if let Some(event) = scene {
            self.broadcast_event(ServerEvent::SceneUpdate(event), Some(&key));
        }

        self.send_to(ServerEvent::UserId(player), &key);

        let scene = self.game.client_scene();
        self.send_to(ServerEvent::SceneChange(scene, layer), &key);
        let perms = self.game.client_perms();
        self.send_to(ServerEvent::PermsChange(perms), &key);

        if player == self.owner {
            if let Ok(event) = self.scene_list().await {
                self.send_to(event, &key);
            }
        }

        self.log(LogLevel::Debug, format!("Client connected ({key})"));

        true
    }

    fn client_active(&self, key: &str) -> bool {
        if let Some(client) = self.clients.get(key) {
            client.active()
        } else {
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

    /// Handles a message from a client. Returns `true` if all is good, or
    /// `false` if the client has been dropped and the socket should be closed.
    pub async fn handle_message(&mut self, message: ClientMessage, from: &str) -> bool {
        // Keep track of last activity. Even a ping will keep the server up.
        self.last_action = time::SystemTime::now();

        if !self.client_active(from) {
            return false;
        }

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

        true
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
        let keys: Vec<(String, i64, String)> = self
            .clients
            .iter()
            .map(|(k, c)| (k.to_owned(), c.user, c.username.clone()))
            .collect();
        for (client_key, user, name) in keys {
            let (_, _, layer) = self.game.add_player(user, &name);
            let scene_change = ServerEvent::SceneChange(self.game.client_scene(), layer);
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

    async fn save_scene(&self) -> anyhow::Result<u128> {
        let start_time = timestamp_us()?;
        let conn = &mut self.acquire_conn().await?;
        if let Some(id) = self.game.project_id() {
            let project = Project::load(conn, id).await?;
            project.update_scene(conn, self.game.server_scene()).await?;
            let end_time = timestamp_us()?;
            Ok(end_time - start_time)
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

                    (scene.title, scene.scene_key)
                })
                .collect();
            Ok(ServerEvent::SceneList(scenes, current))
        } else {
            Err(anyhow!("Failed to find project."))
        }
    }

    fn log<A: AsRef<str>>(&self, level: LogLevel, message: A) {
        log(
            level,
            format!("(Game: {}) {}", self.game.key, message.as_ref()),
        );
    }
}
