use scene::comms::ServerEvent;

pub struct Client<S: ClientSocket> {
    pub user: i64,
    pub username: String,
    socket: Option<S>,
}

impl<S: ClientSocket> Client<S> {
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

    pub fn send(&self, event: &ServerEvent) {
        if let Some(sender) = &self.socket {
            sender.send(event);
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
                .map(|sock| sock.id() == conn_id.unwrap())
                .unwrap_or(false)
        {
            if let Some(sock) = self.socket.take() {
                sock.close();
            }
        }
    }

    pub fn connect(&mut self, socket: S) {
        self.disconnect(None);
        self.socket = Some(socket);
    }
}

pub trait ClientSocket {
    fn id(&self) -> i64;

    fn send(&self, event: &ServerEvent);

    fn close(&self);
}
