use tokio::sync::mpsc::UnboundedSender;
use warp::ws::Message;

pub struct Client {
    pub user: i64,
    sender: Option<UnboundedSender<Message>>,
}

impl Client {
    pub fn new(user: i64) -> Self {
        Client { user, sender: None }
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
