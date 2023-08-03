use actix_ws::Message;
use futures::{
    future::{select, Either},
    StreamExt,
};
use tokio::sync::mpsc::unbounded_channel;

use super::{close_ws, GameHandle};
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
    tokio::task::spawn_local(async move {
        let (send, recv) = unbounded_channel();

        if server.join(user.id, user.username, send).is_err() {
            close_ws(session).await; // Server closed.
            return;
        }

        let mut recv = tokio_stream::wrappers::UnboundedReceiverStream::new(recv);

        loop {
            match select(stream.next(), recv.next()).await {
                Either::Left((Some(Ok(message)), _)) => match message {
                    Message::Binary(bytes) => match bincode::deserialize(&bytes) {
                        Ok(message) => {
                            if server.message(user.id, message).is_err() {
                                close_ws(session).await; // Server closed.
                                break;
                            }
                        }
                        Err(e) => warning(format!("Failed to deserialise client WS message: {e}")),
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
                    if session.binary(msg).await.is_err() {
                        debug(format!("Client ({}) disconnected without reason", user.id));
                        break;
                    }
                }
                Either::Right((None, _)) => {
                    close_ws(session).await; // Server closed.
                    break;
                }
            }
        }
    });
}
