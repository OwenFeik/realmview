use std::collections::HashMap;
use std::time;

use anyhow::anyhow;
use bincode::serialize;
use scene::comms::SceneEvent;
use sqlx::{pool::PoolConnection, SqlitePool};
use warp::ws::Message;

use super::{client::Client, to_message, Game, GameRef, Games};
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
    next_conn_id: i64,
}

impl Server {
    const SAVE_INTERVAL: time::Duration = time::Duration::from_secs(60);
    const INACTIVITY_TIMEOUT: time::Duration = time::Duration::from_secs(1800);

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
            next_conn_id: 1,
        }
    }

    async fn die(&mut self) {
        self.alive = false;

        for client in self.clients.values_mut() {
            client.disconnect(None);
        }

        self.save_scene().await.ok();

        // Delete this game.
        self.games.write().await.remove(&self.game.key);

        self.log(LogLevel::Debug, "Closed server");
    }

    pub async fn start(server: GameRef) {
        server.read().await.log(LogLevel::Debug, "Opened server");
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

    fn _disconnect_client(&mut self, key: &str, conn_id: Option<i64>) {
        if let Some(client) = self.clients.get_mut(key) {
            client.disconnect(conn_id);
            self.log(
                LogLevel::Debug,
                format!("Client ({key}) disconnected (Conn: {conn_id:?})"),
            );
        }
    }

    pub fn disconnect_client(&mut self, key: &str, conn_id: i64) {
        self._disconnect_client(key, Some(conn_id));
    }

    pub async fn connect_client(
        &mut self,
        key: String,
        sender: futures::stream::SplitSink<warp::ws::WebSocket, Message>,
    ) -> Result<i64, ()> {
        let conn_id = self.next_conn_id();
        let (player, name) = if let Some(client) = self.get_client_mut(&key) {
            client.connect(sender, conn_id);
            (client.user, client.username.clone())
        } else {
            return Err(());
        };

        let (perms, scene, layer) = self.game.add_player(player, &name);

        if let Some(event) = perms {
            self.broadcast_event(ServerEvent::PermsUpdate(event), Some(&key));
        }

        if let Some(event) = scene {
            self.broadcast_event(ServerEvent::SceneUpdate(event), Some(&key));
        }

        let scene = self.game.client_scene();
        let perms = self.game.client_perms();
        let mut events = vec![
            ServerEvent::UserId(player),
            ServerEvent::SceneChange(scene),
            ServerEvent::PermsChange(perms),
        ];

        if let Some(layer) = layer {
            events.push(ServerEvent::SelectedLayer(layer));
        }

        self.send_to(ServerEvent::EventSet(events), &key);

        // Separate message as this will only occur after some DB queries.
        if player == self.owner {
            if let Ok(event) = self.scene_list().await {
                self.send_to(event, &key);
            }
        }

        self.log(
            LogLevel::Debug,
            format!("Client ({key}) connected (Conn: {conn_id}) "),
        );

        Ok(conn_id)
    }

    fn next_conn_id(&mut self) -> i64 {
        let id = self.next_conn_id;
        self.next_conn_id += 1;
        id
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

    fn client_key(&self, id: i64) -> Option<&str> {
        self.clients
            .iter()
            .find(|(_, client)| client.user == id)
            .map(|(key, _)| key.as_str())
    }

    fn is_owner(&self, key: &str) -> bool {
        if let Some(client) = self.clients.get(key) {
            client.user == self.owner
        } else {
            false
        }
    }

    fn broadcast_event(&self, event: ServerEvent, exclude: Option<&str>) {
        let message = match to_message(&event) {
            Ok(message) => message,
            Err(e) => {
                self.log(
                    LogLevel::Error,
                    format!("Failed to encode event as message: {e}"),
                );
                return;
            }
        };

        let clients = self.clients.iter();
        if let Some(key) = exclude {
            clients.for_each(|(k, c)| {
                if *key != *k {
                    c.send(message.clone());
                }
            });
        } else {
            clients.for_each(|(_, c)| c.send(message.clone()));
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
                        self.log(LogLevel::Error, format!("Failed to load scene: {e}"));
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
                    let (ok, server_events) = self.game.handle_event(client.user, event.clone());

                    if ok {
                        self.send_approval(message.id, from);
                        self.broadcast_event(ServerEvent::SceneUpdate(event.clone()), Some(from));
                    } else {
                        self.log(LogLevel::Debug, format!("Rejected event: {event:?}"));
                        self.send_rejection(message.id, from);
                    }

                    if let Some(event) = server_events {
                        self.broadcast_event(event, None);
                    }

                    if matches!(event, SceneEvent::LayerRemove(..)) {
                        if let Some((user, layer, event)) = self.game.handle_remove_layer(event) {
                            let exclude = self.client_key(user);

                            if let Some(event) = event {
                                self.broadcast_event(ServerEvent::SceneUpdate(event), exclude);
                            }

                            if let Some(client_key) = exclude {
                                self.send_to(ServerEvent::SelectedLayer(layer), client_key);
                            }
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

            let mut events = vec![
                ServerEvent::SceneChange(self.game.client_scene()),
                ServerEvent::PermsChange(self.game.client_perms()),
            ];

            if let Some(layer) = layer {
                events.push(ServerEvent::SelectedLayer(layer));
            }

            self.send_to(ServerEvent::EventSet(events), &client_key);
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
