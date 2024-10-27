use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use scene::comms::SceneEvent;
use sqlx::{pool::PoolConnection, SqlitePool};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::time::Instant;

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
    pub owner: i64,
    open: Arc<AtomicBool>,
    chan: UnboundedSender<ServerCommand>,
}

impl GameHandle {
    fn send(&self, command: ServerCommand) -> anyhow::Result<()> {
        self.chan.send(command)?;
        Ok(())
    }

    pub fn close(&self) {
        // If this is an error, the other end is already closed, implying the
        // server has stopped. Otherwise the server will stop when it receives
        // our command. In either case, we can mark this game as closed to
        // prevent new players from joining, etc.
        self.send(ServerCommand::Close).ok();
        self.open.store(false, std::sync::atomic::Ordering::Release);
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

    GameHandle {
        owner,
        open,
        chan: send,
    }
}

struct Client {
    user: i64,
    username: String,
    sender: Option<UnboundedSender<Vec<u8>>>,
    check_time: Option<Instant>,
    last_event: Instant,
}

impl Client {
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
    clients: HashMap<i64, Client>,
    last_save: Instant,
    last_action: Instant,
    empty_time: Option<Instant>,
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
        let now = Instant::now();
        Self {
            open,
            owner,
            game: Game::new(project, scene, owner, key),
            pool,
            handle,
            clients: HashMap::new(),
            last_save: now,
            last_action: now,
            empty_time: Some(now),
        }
    }

    async fn run(&mut self) {
        // Interval at which to update client health checks, check if a save is
        // needed, etc.
        const CHECK_INTERVAL: Duration = Duration::from_millis(500);

        // Interval at which to save the game.
        const SAVE_INTERVAL: Duration = Duration::from_secs(600);

        // Time after which to close the game if no event has ocurred.
        const INACTIVITY_TIMEOUT: Duration = Duration::from_secs(1800);

        // Time to keep the game open with no clients.
        const EMPTY_TIMEOUT: Duration = Duration::from_secs(30);

        self.log(LogLevel::Debug, "Opened server");

        loop {
            match tokio::time::timeout(CHECK_INTERVAL, self.handle.recv()).await {
                Ok(Some(command)) => match command {
                    ServerCommand::Close => {
                        self.log(LogLevel::Debug, "Closed by user.");
                        break;
                    }
                    ServerCommand::Join {
                        sender,
                        user,
                        username,
                    } => self.connect_client(user, username, sender).await,
                    ServerCommand::Message { user, message } => {
                        self.handle_message(message, user).await;
                        continue; // Skip checks on a message.
                    }
                },
                Ok(None) => {
                    // All server handles dropped. Closed.
                    self.log(LogLevel::Debug, "Closing as no handles remain.");
                    break;
                }
                Err(_) => {} // Timeout expired. Just check inactivity and save.
            }

            // Check if any clients have died.
            self.health_check();

            if self.last_action.elapsed() >= INACTIVITY_TIMEOUT {
                self.log(LogLevel::Debug, "Closing due to inactivity.");
                break; // Inactive for timeout duration. Close server.
            }

            if let Some(empty_time) = self.empty_time
                && empty_time.elapsed() >= EMPTY_TIMEOUT
            {
                self.log(LogLevel::Debug, "Closing due to emptiness.");
                break; // Inactive for timeout duration. Close server.
            }

            if self.last_save.elapsed() >= SAVE_INTERVAL {
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
        let now = Instant::now();

        // Keep track of last activity. Even a ping will keep the server up.
        self.last_action = now;

        if !self.client_active(from, now) {
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
                if self.game.handle_event(from, event.clone()) {
                    self.send_approval(message.id, from);
                    self.broadcast_event(ServerEvent::SceneUpdate(event.clone()), Some(from));
                } else {
                    self.log(LogLevel::Debug, format!("Rejected event: {event:?}"));
                    self.send_rejection(message.id, from);
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
            Client {
                user,
                username: name.clone(),
                sender: Some(sender),
                check_time: None,
                last_event: Instant::now(),
            },
        );
        self.empty_time = None;

        let (perms, scene, layer) = self.game.add_player(user, &name);

        for event in perms {
            self.broadcast_event(ServerEvent::PermsUpdate(event), Some(user));
        }

        if let Some(event) = scene {
            self.broadcast_event(ServerEvent::SceneUpdate(event), Some(user));
        }

        let scene = self.game.client_scene();
        let perms = self.game.client_perms();
        let mut events = vec![
            ServerEvent::UserId(user),
            ServerEvent::SceneChange(Box::new(scene)),
            ServerEvent::PermsChange(perms),
        ];

        if let Some(layer) = layer {
            events.push(ServerEvent::SelectedLayer(layer));
        }

        if let Some(event) = ServerEvent::set(events) {
            self.send_event(event, user);
        }

        // Separate message as this will only occur after some DB queries.
        if user == self.owner {
            if let Ok(event) = self.scene_list().await {
                self.send_event(event, user);
            }
        }

        self.log(
            LogLevel::Debug,
            format!("Client ({user}) connected. Layer: {layer:?}."),
        );
    }

    fn disconnect_client(&mut self, user: i64) {
        self.send_event(ServerEvent::Disconnect, user);
        if self.clients.remove_entry(&user).is_some() {
            self.log(LogLevel::Debug, format!("Client ({user}) disconnected."));
        }
        if self.clients.is_empty() {
            self.empty_time = Some(Instant::now());
        }
    }

    fn client_active(&mut self, user: i64, time: Instant) -> bool {
        if let Some(client) = self.clients.get_mut(&user) {
            client.last_event = time;
            client.active()
        } else {
            false
        }
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

    fn health_check(&mut self) {
        /// Time to allow a client to be quiet for before sending a heartbeat.
        const QUIET_TIME: Duration = Duration::from_secs(5);

        /// Time to wait after sending a heartbeat before presuming a client
        /// dead and disconnecting it.
        const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(30);

        let now = Instant::now();
        let mut clients_to_disconnect = Vec::new();
        for client in self.clients.values_mut() {
            if let Some(since) = now.checked_duration_since(client.last_event)
                && since >= QUIET_TIME
            {
                if let Some(since) = client
                    .check_time
                    .and_then(|t| now.checked_duration_since(t))
                {
                    if since >= HEARTBEAT_TIMEOUT {
                        clients_to_disconnect.push(client.user);
                    }
                } else {
                    client.check_time = Some(now);
                    if let Ok(message) = bincode::serialize(&ServerEvent::HealthCheck) {
                        client.send(message);
                    }
                }
            } else {
                client.check_time = None;
            }
        }

        for user in clients_to_disconnect {
            self.log(LogLevel::Info, format!("User ({user}) timed out."));
            self.disconnect_client(user);
        }
    }

    fn serialise(&self, event: ServerEvent) -> Option<Vec<u8>> {
        if let Ok(message) = bincode::serialize(&event) {
            Some(message)
        } else {
            log(LogLevel::Error, "Failed to encode server event as message.");
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
                format!("Saved scene. Save duration: {duration}us."),
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
            self.last_save = Instant::now();
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
                ServerEvent::SceneChange(Box::new(self.game.client_scene())),
                ServerEvent::PermsChange(self.game.client_perms()),
            ];

            if let Some(layer) = layer {
                events.push(ServerEvent::SelectedLayer(layer));
            }

            if let Some(event) = ServerEvent::set(events) {
                self.send_event(event, user);
            }
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
                    if current_scene_id == scene.id {
                        current.clone_from(&scene.scene_key);
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
