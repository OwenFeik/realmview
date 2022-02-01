use std::sync::Arc;

use futures::{FutureExt, StreamExt};
use tokio::sync::{mpsc, RwLock};
use warp::ws::{Message, WebSocket};

use scene::Scene;

struct Client {
    user: i64,
    sender: Option<mpsc::UnboundedSender<Result<Message, warp::Error>>>,
}

struct Session {
    scene: Scene,
}

impl Session {
    fn event(&self, user: i64) {
        println!("Event from user {}", user);
    }
}

async fn client_connection(ws: WebSocket, mut client: Client, session: Arc<RwLock<Session>>) {
    let (client_ws_send, mut client_ws_recv) = ws.split();
    let (client_send, client_recv) = mpsc::unbounded_channel();
    let client_recv = tokio_stream::wrappers::UnboundedReceiverStream::new(client_recv);
    tokio::task::spawn(client_recv.forward(client_ws_send).map(|result| {
        if let Err(e) = result {
            eprintln!("error sending websocket msg: {}", e);
        }
    }));

    client.sender = Some(client_send);

    while let Some(result) = client_ws_recv.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("error receiving ws message: {}", e);
                break;
            }
        };

        session.read().await.event(client.user);
    }
}
