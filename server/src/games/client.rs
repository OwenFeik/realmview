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

    /// Disconnect a client (optionally specifiying the connection ID to
    /// terminate). If no connection ID is specified, the connection will
    /// always be closed.
    pub fn disconnect(&mut self, conn_id: Option<i64>) {
        if conn_id.is_none()
            || self
                .socket
                .as_ref()
                .map(|sock| sock.conn_id == conn_id.unwrap())
                .unwrap_or(false)
        {
            if let Some(sock) = self.socket.take() {
                sock.close();
            }
        }
    }

    pub fn connect(&mut self, socket: SplitSink<WebSocket, Message>, conn_id: i64) {
        self.disconnect(None);
        self.socket = Some(ClientSocket::new(socket, conn_id));
    }
}

struct ClientSocket {
    conn_id: i64,
    sender: UnboundedSender<Message>,
    connected: Arc<AtomicBool>,
}

impl ClientSocket {
    const CHECK_DISCONNECT_INTERVAL: Duration = Duration::from_secs(1);

    fn new(mut socket: SplitSink<WebSocket, Message>, conn_id: i64) -> Self {
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
                        // This means taht the stream has been dropped, we
                        // should close the socket.
                        break;
                    }
                    Err(_) => {} // Timeout expired, just disconnected check.
                }

                // Check that connected flag is still active.
                if !connected_ref.load(Ordering::Relaxed) {
                    break;
                }
            }

            Self::close_socket(socket, connected_ref).await;
        });

        Self {
            conn_id,
            sender,
            connected,
        }
    }

    async fn close_socket(mut socket: SplitSink<WebSocket, Message>, connected: Arc<AtomicBool>) {
        const CLOSE_NORMAL: u16 = 1000;
        const CLOSE_REASON: &str = "gameover";

        let message = Message::close_with(CLOSE_NORMAL, CLOSE_REASON);
        if let Err(e) = socket.send(message).await {
            error(format!("Failed send disconnect message: {e}"));
        } else {
            debug("WebSocket sender closed");
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
