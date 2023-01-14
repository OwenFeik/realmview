use tokio::sync::mpsc::UnboundedSender;
use warp::ws::Message;

pub struct Client {
    pub user: i64,
    pub username: String,
    sender: Option<UnboundedSender<Message>>,
}

impl Client {
    pub fn new(user: i64, username: String) -> Self {
        Client {
            user,
            username,
            sender: None,
        }
    }

    pub fn send(&self, message: Message) {
        if let Some(sender) = &self.sender {
            sender.send(message).ok();
        }
    }

    pub fn set_sender(&mut self, sender: UnboundedSender<Message>) {
        self.sender = Some(sender);
    }
}
