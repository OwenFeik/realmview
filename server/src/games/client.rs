use actix_ws::{CloseReason, Message};
use futures::{
    future::{select, Either},
    StreamExt,
};
use tokio::sync::mpsc::unbounded_channel;

use super::GameHandle;
use crate::{
    models::User,
    utils::{debug, warning},
};

pub fn connect_game_client(
    user: User,
    server: GameHandle,
    mut session: actix_ws::Session,
    mut stream: actix_ws::MessageStream,
) {
    const CLOSE_REASON: &str = "gameover";

    tokio::task::spawn_local(async move {
        let (send, recv) = unbounded_channel();

        server.join(user.id, user.username, send);

        let mut recv = tokio_stream::wrappers::UnboundedReceiverStream::new(recv);

        loop {
            match select(stream.next(), recv.next()).await {
                Either::Left((Some(Ok(message)), _)) => match message {
                    Message::Binary(bytes) => match bincode::deserialize(&bytes) {
                        Ok(message) => {
                            server.message(user.id, message);
                        }
                        Err(e) => warning("Failed to deserialise client WS message"),
                    },
                    Message::Close(reason) => {
                        debug(format!(
                            "Client ({}) disconnected. Reason: {reason:?}",
                            user.id
                        ));
                        break;
                    }
                    msg => warning(format!("Unexpected WS message: {msg:?}")),
                },
                Either::Left((Some(Err(e)), _)) => {
                    warning(format!("WS protocol error: {e}"));
                }
                Either::Left((None, _)) => {
                    debug(format!("Client ({}) disconnected without reason", user.id));
                    break;
                }
                Either::Right((Some(msg), _)) => {
                    if let Err(_) = session.binary(msg).await {
                        debug(format!("Client ({}) disconnected without reason", user.id));
                        break;
                    }
                }
                Either::Right((None, _)) => {
                    // Server closed.
                    session.close(Some(CloseReason {
                        code: actix_ws::CloseCode::Normal,
                        description: Some(CLOSE_REASON.to_string()),
                    }));
                    break;
                }
            }
        }
    });
}
