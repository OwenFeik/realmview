use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use futures::{stream::SplitSink, SinkExt, StreamExt, TryFutureExt};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use warp::ws::{Message, WebSocket};

use super::to_message;
use crate::utils::{debug, error};

pub struct Client {
    pub user: i64,
    pub username: String,
    socket: Option<ClientSocket>,
}

impl Client {
    pub fn new(user: i64, username: String) -> Self {
        Client {
            user,
            username,
            socket: None,
        }
    }

    pub fn active(&self) -> bool {
        self.socket.is_some()
    }

    pub fn send(&self, message: Message) {
        if let Some(sender) = &self.socket {
            sender.send(message);
        }
    }

    pub fn disconnect(&mut self) {
        // TODO this kicks new client too for some reason.
        // if let Ok(message) = to_message(&scene::comms::ServerEvent::GameOver) {
        //     self.send(message);
        // } else {
        //     error("Failed to encode ServerEvent::GameOver as message.")
        // }

        if let Some(sock) = self.socket.take() {
            sock.close();
        }
    }

    pub fn connect(&mut self, socket: SplitSink<WebSocket, Message>) {
        self.disconnect();
        self.socket = Some(ClientSocket::new(socket));
    }
}

struct ClientSocket {
    sender: UnboundedSender<Message>,
    connected: Arc<AtomicBool>,
}

impl ClientSocket {
    const CHECK_DISCONNECT_INTERVAL: Duration = Duration::from_secs(60);

    fn new(mut socket: SplitSink<WebSocket, Message>) -> Self {
        let connected = Arc::new(AtomicBool::new(true));
        let (sender, receiver) = unbounded_channel();
        let mut receiver_stream = tokio_stream::wrappers::UnboundedReceiverStream::new(receiver);
        let connected_ref = connected.clone();
        tokio::task::spawn(async move {
            loop {
                match tokio::time::timeout(Self::CHECK_DISCONNECT_INTERVAL, receiver_stream.next())
                    .await
                {
                    Ok(Some(msg)) => {
                        socket
                            .send(msg)
                            .unwrap_or_else(|e| error(format!("Error sending websocket msg: {e}")))
                            .await
                    }
                    Ok(None) => {
                        Self::close_socket(socket, connected_ref).await;
                        break;
                    }
                    Err(_) => {} // Timeout expired, just disconnected check.
                }

                // Check that connected flag is still active.
                if !connected_ref.load(Ordering::Relaxed) {
                    Self::close_socket(socket, connected_ref).await;
                    break;
                }
            }
        });

        Self { sender, connected }
    }

    async fn close_socket(mut socket: SplitSink<WebSocket, Message>, connected: Arc<AtomicBool>) {
        if let Err(e) = socket.close().await {
            error(format!("Failed to disconnect client websocket: {e}"));
        } else {
            debug("Websocket sender closed.");
        }
        connected.store(false, Ordering::Relaxed);
    }

    fn send(&self, message: Message) {
        if let Err(e) = self.sender.send(message) {
            error(format!("Error sending stream msg: {e}"));
        }
    }

    fn close(self) {
        self.connected.store(false, Ordering::Relaxed);
    }
}
