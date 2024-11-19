use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use scene::comms::SceneEvent;
use sqlx::{pool::PoolConnection, SqlitePool};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::time::Instant;
use uuid::Uuid;

use super::game::{Game, GameKey};
use crate::models::User;
use crate::{
    models::Project,
    scene::comms::{ClientEvent, ClientMessage, ServerEvent},
    utils::{log, timestamp_us, LogLevel, Res},
};

#[derive(Debug)]
pub enum ServerCommand {
    Close,
    Join {
        user: Uuid,
        username: String,
        sender: UnboundedSender<Vec<u8>>,
    },
    Message {
        user: Uuid,
        message: ClientMessage,
    },
}

#[derive(Clone)]
pub struct GameHandle {
    pub owner: Uuid,
    open: Arc<AtomicBool>,
    chan: UnboundedSender<ServerCommand>,
}

impl GameHandle {
    fn send(&self, command: ServerCommand) -> Res<()> {
        self.chan.send(command).map_err(|e| e.to_string())?;
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

    pub fn join(&self, user: Uuid, username: String, sender: UnboundedSender<Vec<u8>>) -> Res<()> {
        self.send(ServerCommand::Join {
            user,
            username,
            sender,
        })
    }

    pub fn message(&self, user: Uuid, message: ClientMessage) -> Res<()> {
        self.send(ServerCommand::Message { user, message })
    }
}

pub fn launch(
    key: GameKey,
    owner: User,
    project: scene::Project,
    scene: Uuid,
    pool: SqlitePool,
) -> GameHandle {
    let owner_uuid = owner.uuid;

    let open = Arc::new(AtomicBool::new(true));
    let (send, recv) = unbounded_channel();

    let open_ref = open.clone();
    tokio::task::spawn(async move {
        let mut server = Server::new(open_ref, key, owner, project, scene, pool, recv);
        server.run().await;
    });

    GameHandle {
        owner: owner_uuid,
        open,
        chan: send,
    }
}

struct Client {
    user: Uuid,
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
    owner: User,
    game: Game,
    pool: SqlitePool,
    handle: UnboundedReceiver<ServerCommand>,
    clients: HashMap<Uuid, Client>,
    last_save: Instant,
    last_action: Instant,
    empty_time: Option<Instant>,
}

impl Server {
    fn new(
        open: Arc<AtomicBool>,
        key: GameKey,
        owner: User,
        project: scene::Project,
        scene: Uuid,
        pool: SqlitePool,
        handle: UnboundedReceiver<ServerCommand>,
    ) -> Self {
        let now = Instant::now();
        let owner_uuid = owner.uuid;
        Self {
            open,
            owner,
            game: Game::new(project, scene, owner_uuid, key),
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
                self.save().await; // Save interval elapsed, save scene.
            }
        }

        self.broadcast_event(ServerEvent::GameOver, None);
        self.clients.clear();
        self.save().await;
        self.open.store(false, std::sync::atomic::Ordering::Release);
        self.log(LogLevel::Debug, "Closed server");
    }

    /// Handles a message from a client. Returns `true` if all is good, or
    /// `false` if the client has been dropped and the socket should be closed.
    async fn handle_message(&mut self, message: ClientMessage, from: Uuid) {
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
            ClientEvent::SceneChange(scene) => {
                if self.game.owner_is(from) {
                    if let Err(e) = self.game.switch_to_scene(scene) {
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

                // Handle layer removal by ensuring that the player still has a layer.i64
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

    async fn connect_client(&mut self, user: Uuid, name: String, sender: UnboundedSender<Vec<u8>>) {
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
        if user == self.owner.uuid {
            let (list, selected) = self.game.scene_list();
            self.send_event(ServerEvent::SceneList(list, selected), user);
        }

        self.log(
            LogLevel::Debug,
            format!("Client ({user}) connected. Layer: {layer:?}."),
        );
    }

    fn disconnect_client(&mut self, user: Uuid) {
        self.send_event(ServerEvent::Disconnect, user);
        if self.clients.remove_entry(&user).is_some() {
            self.log(LogLevel::Debug, format!("Client ({user}) disconnected."));
        }
        if self.clients.is_empty() {
            self.empty_time = Some(Instant::now());
        }
    }

    fn client_active(&mut self, user: Uuid, time: Instant) -> bool {
        if let Some(client) = self.clients.get_mut(&user) {
            client.last_event = time;
            client.active()
        } else {
            false
        }
    }

    fn send_approval(&mut self, event_id: i64, user: Uuid) {
        self.send_event(ServerEvent::Approval(event_id), user);
    }

    fn send_rejection(&mut self, event_id: i64, user: Uuid) {
        self.send_event(ServerEvent::Rejection(event_id), user);
    }

    fn send_event(&mut self, event: ServerEvent, user: Uuid) {
        let Some(message) = self.serialise(event) else {
            return;
        };

        if let Some(client) = self.clients.get_mut(&user) {
            client.send(message);
        }
    }

    fn broadcast_event(&mut self, event: ServerEvent, exclude: Option<Uuid>) {
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

    async fn _save(&self) -> Res<()> {
        let start_time = timestamp_us()?;
        let conn = &mut self.acquire_conn().await?;
        Project::save(conn, &self.owner, self.game.copy_project()).await?;
        let duration = timestamp_us()? - start_time;
        self.log(
            LogLevel::Debug,
            format!("Saved scene. Save duration: {duration}us."),
        );
        Ok(())
    }

    async fn save(&mut self) {
        if let Err(e) = self._save().await {
            self.log(LogLevel::Error, format!("Failed to save project: {e}"));
        } else {
            self.last_save = Instant::now();
        }
    }

    async fn switch_to_scene(&mut self, scene: Uuid) {
        // Replace the local scene with the new one
        if self.game.switch_to_scene(scene).is_err() {
            return;
        }

        // Send the new scene and perms to all clients
        let keys: Vec<(Uuid, String)> = self
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

    async fn acquire_conn(&self) -> Res<PoolConnection<sqlx::Sqlite>> {
        self.pool
            .acquire()
            .await
            .map_err(|e| format!("Failed to acquire connection: {e}"))
    }
}
