use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::anyhow;
use scene::comms::SceneEvent;
use sqlx::{pool::PoolConnection, SqlitePool};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use super::game::Game;
use crate::{
    models::{Project, SceneRecord},
    scene::{
        comms::{ClientEvent, ClientMessage, ServerEvent},
        Scene,
    },
    utils::{log, timestamp_us, LogLevel},
};

#[derive(Debug)]
pub enum ServerCommand {
    Close,
    Join {
        user: i64,
        username: String,
        sender: UnboundedSender<Vec<u8>>,
    },
    Message {
        user: i64,
        message: ClientMessage,
    },
}

#[derive(Clone)]
pub struct GameHandle {
    open: Arc<AtomicBool>,
    chan: UnboundedSender<ServerCommand>,
}

impl GameHandle {
    fn send(&self, command: ServerCommand) -> anyhow::Result<()> {
        self.chan.send(command)?;
        Ok(())
    }

    pub fn open(&self) -> bool {
        self.open.load(std::sync::atomic::Ordering::Acquire)
    }

    pub fn join(
        &self,
        user: i64,
        username: String,
        sender: UnboundedSender<Vec<u8>>,
    ) -> anyhow::Result<()> {
        self.send(ServerCommand::Join {
            user,
            username,
            sender,
        })
    }

    pub fn message(&self, user: i64, message: ClientMessage) -> anyhow::Result<()> {
        self.send(ServerCommand::Message { user, message })
    }
}

pub fn launch(key: String, owner: i64, project: i64, scene: Scene, pool: SqlitePool) -> GameHandle {
    let open = Arc::new(AtomicBool::new(true));
    let (send, recv) = unbounded_channel();

    let open_ref = open.clone();
    tokio::task::spawn(async move {
        let mut server = Server::new(open_ref, &key, owner, project, scene, pool, recv);
        server.run().await;
    });

    GameHandle { open, chan: send }
}

struct NewClient {
    user: i64,
    username: String,
    sender: Option<UnboundedSender<Vec<u8>>>,
}

impl NewClient {
    fn send(&mut self, message: Vec<u8>) {
        if let Some(sender) = &self.sender {
            if sender.send(message).is_err() {
                self.sender = None;
            }
        }
    }

    fn active(&self) -> bool {
        match &self.sender {
            Some(sender) => !sender.is_closed(),
            None => false,
        }
    }
}

struct Server {
    open: Arc<AtomicBool>,
    owner: i64,
    game: Game,
    pool: SqlitePool,
    handle: UnboundedReceiver<ServerCommand>,
    clients: HashMap<i64, NewClient>,
    last_save: SystemTime,
    last_action: SystemTime,
}

impl Server {
    fn new(
        open: Arc<AtomicBool>,
        key: &str,
        owner: i64,
        project: i64,
        scene: Scene,
        pool: SqlitePool,
        handle: UnboundedReceiver<ServerCommand>,
    ) -> Self {
        Self {
            open,
            owner,
            game: Game::new(project, scene, owner, key),
            pool,
            handle,
            clients: HashMap::new(),
            last_save: SystemTime::now(),
            last_action: SystemTime::now(),
        }
    }

    async fn run(&mut self) {
        const CHECK_INTERVAL: Duration = Duration::from_secs(10);
        const SAVE_INTERVAL: Duration = Duration::from_secs(600);
        const INACTIVITY_TIMEOUT: Duration = Duration::from_secs(1800);

        self.log(LogLevel::Debug, "Opened server");

        loop {
            match tokio::time::timeout(CHECK_INTERVAL, self.handle.recv()).await {
                Ok(Some(command)) => match command {
                    ServerCommand::Close => break,
                    ServerCommand::Join {
                        sender,
                        user,
                        username,
                    } => self.connect_client(user, username, sender).await,
                    ServerCommand::Message { user, message } => {
                        self.handle_message(message, user).await
                    }
                },
                Ok(None) => break, // All server handles dropped. Closed.
                Err(_) => {}       // Timeout expired. Just check inactivity and save.
            }

            if duration_elapsed(self.last_action, INACTIVITY_TIMEOUT) {
                break; // Inactive for timeout duration. Close server.
            }

            if duration_elapsed(self.last_save, SAVE_INTERVAL) {
                self.save_scene().await; // Save interval elapsed, save scene.
            }
        }

        self.broadcast_event(ServerEvent::GameOver, None);
        self.clients.clear();
        self.save_scene().await;
        self.open.store(false, std::sync::atomic::Ordering::Release);
        self.log(LogLevel::Debug, "Closed server");
    }

    /// Handles a message from a client. Returns `true` if all is good, or
    /// `false` if the client has been dropped and the socket should be closed.
    async fn handle_message(&mut self, message: ClientMessage, from: i64) {
        // Keep track of last activity. Even a ping will keep the server up.
        self.last_action = SystemTime::now();

        if !self.client_active(from) {
            self.log(
                LogLevel::Debug,
                format!("Client ({from}) messaged while disconnected"),
            );
            return;
        }

        match message.event {
            ClientEvent::Ping => {
                self.send_approval(message.id, from);
            }
            ClientEvent::SceneChange(scene_key) => {
                if self.game.owner_is(from) {
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
                let (ok, server_events) = self.game.handle_event(from, event.clone());

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

                // Handle layer removal by ensuring that the player still has a layer.
                if matches!(event, SceneEvent::LayerRemove(..)) {
                    if let Some((user, layer, event)) = self.game.handle_remove_layer(event) {
                        if let Some(event) = event {
                            self.broadcast_event(ServerEvent::SceneUpdate(event), None);
                        }

                        self.send_event(ServerEvent::SelectedLayer(layer), user);
                    }
                }
            }
        };
    }

    async fn connect_client(&mut self, user: i64, name: String, sender: UnboundedSender<Vec<u8>>) {
        self.disconnect_client(user);
        self.clients.insert(
            user,
            NewClient {
                user,
                username: name.clone(),
                sender: Some(sender),
            },
        );

        let (perms, scene, layer) = self.game.add_player(user, &name);

        if let Some(event) = perms {
            self.broadcast_event(ServerEvent::PermsUpdate(event), Some(user));
        }

        if let Some(event) = scene {
            self.broadcast_event(ServerEvent::SceneUpdate(event), Some(user));
        }

        let scene = self.game.client_scene();
        let perms = self.game.client_perms();
        let mut events = vec![
            ServerEvent::UserId(user),
            ServerEvent::SceneChange(scene),
            ServerEvent::PermsChange(perms),
        ];

        if let Some(layer) = layer {
            events.push(ServerEvent::SelectedLayer(layer));
        }

        self.send_event(ServerEvent::EventSet(events), user);

        // Separate message as this will only occur after some DB queries.
        if user == self.owner {
            if let Ok(event) = self.scene_list().await {
                self.send_event(event, user);
            }
        }

        self.log(LogLevel::Debug, format!("Client ({user}) connected"));
    }

    fn disconnect_client(&mut self, user: i64) {
        self.send_event(ServerEvent::GameOver, user);
        if self.clients.remove_entry(&user).is_some() {
            self.log(LogLevel::Debug, format!("Client ({user}) disconnected"));
        }
    }

    fn client_active(&self, user: i64) -> bool {
        self.clients
            .get(&user)
            .map(NewClient::active)
            .unwrap_or(false)
    }

    fn send_approval(&mut self, event_id: i64, user: i64) {
        self.send_event(ServerEvent::Approval(event_id), user);
    }

    fn send_rejection(&mut self, event_id: i64, user: i64) {
        self.send_event(ServerEvent::Rejection(event_id), user);
    }

    fn send_event(&mut self, event: ServerEvent, user: i64) {
        let Some(message) = self.serialise(event) else {
            return;
        };

        if let Some(client) = self.clients.get_mut(&user) {
            client.send(message);
        }
    }

    fn broadcast_event(&mut self, event: ServerEvent, exclude: Option<i64>) {
        let Some(message) = self.serialise(event) else {
            return;
        };

        if let Some(user) = exclude {
            self.clients
                .iter_mut()
                .filter(|(id, _)| **id != user)
                .for_each(|(_, client)| {
                    client.send(message.clone());
                })
        } else {
            self.clients
                .values_mut()
                .for_each(|client| client.send(message.clone()));
        }
    }

    fn serialise(&self, event: ServerEvent) -> Option<Vec<u8>> {
        if let Ok(message) = bincode::serialize(&event) {
            Some(message)
        } else {
            log(LogLevel::Error, "Failed to encode server event as message");
            None
        }
    }

    fn log<A: AsRef<str>>(&self, level: LogLevel, message: A) {
        log(
            level,
            format!("(Game: {}) {}", self.game.key, message.as_ref()),
        );
    }

    async fn load_scene(&mut self, scene_key: &str) -> anyhow::Result<()> {
        let conn = &mut self.acquire_conn().await?;
        let record = SceneRecord::load_from_key(conn, scene_key).await?;
        let scene = record.load_scene(conn).await?;

        self.replace_scene(scene).await;

        Ok(())
    }

    async fn _save_scene(&self) -> anyhow::Result<()> {
        let start_time = timestamp_us()?;
        let conn = &mut self.acquire_conn().await?;
        if let Some(id) = self.game.project_id() {
            let project = Project::load(conn, id).await?;
            project.update_scene(conn, self.game.server_scene()).await?;
            let duration = timestamp_us()? - start_time;
            self.log(
                LogLevel::Debug,
                format!("Saved scene. Save duration: {duration}us"),
            );
            Ok(())
        } else {
            Err(anyhow!("Scene has no project."))
        }
    }

    async fn save_scene(&mut self) {
        if let Err(e) = self._save_scene().await {
            self.log(LogLevel::Error, format!("Failed to save scene: {e}"));
        } else {
            self.last_save = SystemTime::now();
        }
    }

    async fn replace_scene(&mut self, scene: Scene) {
        self.save_scene().await;

        // Replace the local scene with the new one
        self.game.replace_scene(scene.clone(), self.owner);

        // Send the new scene and perms to all clients
        let keys: Vec<(i64, String)> = self
            .clients
            .iter()
            .map(|(u, c)| (*u, c.username.clone()))
            .collect();
        for (user, name) in keys {
            let (_, _, layer) = self.game.add_player(user, &name);

            let mut events = vec![
                ServerEvent::SceneChange(self.game.client_scene()),
                ServerEvent::PermsChange(self.game.client_perms()),
            ];

            if let Some(layer) = layer {
                events.push(ServerEvent::SelectedLayer(layer));
            }

            self.send_event(ServerEvent::EventSet(events), user);
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

    async fn acquire_conn(&self) -> anyhow::Result<PoolConnection<sqlx::Sqlite>> {
        self.pool
            .acquire()
            .await
            .map_err(|e| anyhow!("Failed to acquire connection: {e}"))
    }
}

fn duration_elapsed(start: SystemTime, duration: Duration) -> bool {
    start.elapsed().map(|d| d >= duration).unwrap_or(false)
}
