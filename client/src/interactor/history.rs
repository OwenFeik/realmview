use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};

use scene::comms::{ClientEvent, ClientMessage, SceneEvent, ServerEvent};
use scene::Id;

use crate::client::Client;

pub struct History {
    client: Option<Client>,
    history: Vec<SceneEvent>,
    redo_history: Vec<SceneEvent>,
    issued_events: Vec<ClientMessage>,
}

impl History {
    pub fn new(client: Option<Client>) -> Self {
        Self {
            client,
            history: vec![],
            redo_history: vec![],
            issued_events: vec![],
        }
    }

    pub fn server_events(&self) -> Option<Vec<ServerEvent>> {
        self.client.as_ref().map(|client| client.events())
    }

    pub fn take_event(&mut self, id: Id) -> Option<SceneEvent> {
        let i = self.issued_events.iter().position(|c| c.id == id)?;
        if let ClientEvent::SceneUpdate(event) = self.issued_events.remove(i).event {
            Some(event)
        } else {
            None
        }
    }

    pub fn approve_event(&mut self, id: Id) {
        self.issued_events.retain(|c| c.id != id);
    }

    /// Internal common backend for `issue_event` and `issue_event_no_history`,
    /// handles creating a `ClientMessage` from a `SceneEvent` with a unique ID
    /// and sending it to the server. If there is no `Client`, this is a no-op.
    fn _issue_event(&mut self, event: SceneEvent) {
        static EVENT_ID: AtomicI64 = AtomicI64::new(1);

        // Queue event to be sent to server
        if let Some(client) = &self.client {
            let message = ClientMessage {
                id: EVENT_ID.fetch_add(1, Ordering::Relaxed),
                event: ClientEvent::SceneUpdate(event),
            };
            client.send_message(&message);
            self.issued_events.push(message);
        }
    }

    /// Issue an event, publishing it to the server and adding it to the
    /// history stack. Should be called with every event produced from the
    /// scene to ensure consistency with the server.
    pub fn issue_event(&mut self, event: SceneEvent) {
        if self.client.is_some() {
            self._issue_event(event.clone());
        }

        // When adding a new entry to the history, all undone events are lost.
        self.redo_history.clear();
        self.history.push(event);
    }

    /// Issue an event to the server without affecting the history stack.
    pub fn issue_event_no_history(&mut self, event: SceneEvent) {
        self._issue_event(event);
    }

    pub fn issue_redo(&mut self, opt: Option<SceneEvent>) {
        if let Some(event) = opt {
            self.redo_history.push(event);
        }
    }

    pub fn pop(&mut self) -> Option<SceneEvent> {
        self.history.pop()
    }

    pub fn pop_redo(&mut self) -> Option<SceneEvent> {
        self.redo_history.pop()
    }

    pub fn start_move_group(&mut self) {
        self.history.push(SceneEvent::Dummy);
    }

    fn consume_history_until<F: FnMut(&SceneEvent) -> bool>(&mut self, mut pred: F) {
        while let Some(e) = self.history.pop() {
            if !pred(&e) {
                if !matches!(e, SceneEvent::Dummy) {
                    self.history.push(e);
                }
                break;
            }
        }
    }

    fn group_moves_single(&mut self, last: SceneEvent) {
        let (sprite, mut start, finish) = if let SceneEvent::SpriteMove(id, from, to) = last {
            (id, from, to)
        } else {
            return;
        };

        self.consume_history_until(|e| {
            if let SceneEvent::SpriteMove(id, from, _) = e {
                if *id == sprite {
                    start = *from;
                    return true;
                }
            }
            false
        });

        self.history
            .push(SceneEvent::SpriteMove(sprite, start, finish));
    }

    fn group_moves_drawing(&mut self, last: SceneEvent) {
        let sprite = if let SceneEvent::SpriteDrawingFinish(id) = last {
            id
        } else {
            return;
        };

        let mut opt = None;
        self.consume_history_until(|e| match e {
            SceneEvent::SpriteDrawingPoint(id, ..) => *id == sprite,
            SceneEvent::SpriteNew(s, ..) => {
                if s.id == sprite {
                    opt = Some(e.clone());
                }
                false
            }
            _ => false,
        });

        if let Some(event) = opt {
            self.history.push(event);
        }
    }

    fn group_moves_set(&mut self, last: SceneEvent) {
        self.history.push(last);
        let mut moves = HashMap::new();

        self.consume_history_until(|e| {
            if let SceneEvent::EventSet(v) = e {
                for event in v {
                    if let SceneEvent::SpriteMove(id, from, _) = event {
                        if let Some(SceneEvent::SpriteMove(_, start, _)) = moves.get_mut(id) {
                            *start = *from;
                        } else {
                            moves.insert(*id, event.clone());
                        }
                    }
                }
                true
            } else {
                false
            }
        });

        self.history.push(SceneEvent::EventSet(
            moves.into_values().collect::<Vec<SceneEvent>>(),
        ));
    }

    pub fn end_move_group(&mut self) {
        let opt = self.history.pop();
        if let Some(event) = opt {
            match event {
                SceneEvent::SpriteDrawingFinish(..) => self.group_moves_drawing(event),
                SceneEvent::SpriteMove(..) => self.group_moves_single(event),
                SceneEvent::EventSet(..) => self.group_moves_set(event),
                _ => self.history.push(event),
            };
        }
    }
}