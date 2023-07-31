use actix::{Actor, StreamHandler};
use actix_web_actors::ws;
use scene::comms::ServerEvent;
use tokio::sync::mpsc::unbounded_channel;

use super::client::ClientSocket;

pub struct ActixWs;

impl ActixWs {
    pub fn new() {
        let (send, recv) = unbounded_channel();
    }
}

impl Actor for ActixWs {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for ActixWs {
    fn handle(&mut self, item: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {}
}

impl ClientSocket for ActixWs {
    fn id(&self) -> i64 {
        todo!()
    }

    fn send(&self, event: &ServerEvent) {
        todo!()
    }

    fn close(&self) {
        todo!()
    }
}
