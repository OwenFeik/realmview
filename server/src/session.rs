use std::collections::HashMap;
use std::sync::Arc;
use warp::ws::Message;

use tokio::sync::{mpsc, RwLock};

use scene::Scene;

type Clients = Arc<RwLock<HashMap<String, Client>>>;

struct Client {
    user: i64,
    sender: Option<mpsc::UnboundedSender<Result<Message, warp::Error>>>
}


struct Session {
    scene: Scene,
    clients: Clients
}

impl Session {
    fn event() {

    }
}
