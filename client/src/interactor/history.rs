use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};

use scene::comms::{ClientEvent, ClientMessage, SceneEvent, ServerEvent};
use scene::Id;

use crate::client::Client;

pub struct History {
    client: Option<Client>,
    modified: bool,
    history: Vec<SceneEvent>,
    redo_history: Vec<SceneEvent>,
    issued_events: Vec<ClientMessage>,
}

impl History {
    pub fn new(client: Option<Client>) -> Self {
        Self {
            client,
            modified: false,
            history: vec![],
            redo_history: vec![],
            issued_events: vec![],
        }
    }

    pub fn server_events(&mut self) -> Option<Vec<ServerEvent>> {
        self.client.as_mut().map(|client| client.events())
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

    pub fn save_required(&self) -> bool {
        // If client is present, server scene is canonical and will be saved
        // automatically.
        self.modified && self.client.is_none()
    }

    pub fn clear_modified(&mut self) {
        self.modified = false;
    }

    /// Creates a `ClientMessage` with a unique ID and sends it to the server.
    /// If there is no `Client`, this is a no-op.
    fn issue_message(&mut self, event: ClientEvent) {
        static EVENT_ID: AtomicI64 = AtomicI64::new(1);

        // Queue event to be sent to server
        if let Some(client) = &mut self.client {
            let message = ClientMessage {
                id: EVENT_ID.fetch_add(1, Ordering::Relaxed),
                event,
            };
            client.send_message(&message);
            self.issued_events.push(message);
        }
    }

    fn is_pointless(event: &SceneEvent) -> bool {
        match event {
            SceneEvent::EventSet(events) => events.is_empty(), // Empty event set. Useless.
            _ => false,
        }
    }

    /// Internal common backend for `issue_event` and `issue_event_no_history`,
    /// handles creating a `ClientEvent` from a `SceneEvent` and pushing along
    /// to be sent in a message.
    fn _issue_event(&mut self, event: SceneEvent) {
        self.issue_message(ClientEvent::SceneUpdate(event));
    }

    /// Issue an event, publishing it to the server and adding it to the
    /// history stack. Should be called with every event produced from the
    /// scene to ensure consistency with the server.
    pub fn issue_event(&mut self, event: SceneEvent) {
        if Self::is_pointless(&event) {
            return;
        }

        if self.client.is_some() {
            self._issue_event(event.clone());
        }

        // When adding a new entry to the history, all undone events are lost.
        self.redo_history.clear();
        self.history.push(event);

        self.modified = true;
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

    fn drain_history_until<F: FnMut(&SceneEvent) -> bool>(
        &mut self,
        mut pred: F,
    ) -> Vec<SceneEvent> {
        let mut drained = Vec::new();
        while let Some(e) = self.history.pop() {
            if pred(&e) {
                drained.push(e);
            } else {
                if !matches!(e, SceneEvent::Dummy) {
                    self.history.push(e);
                }
                break;
            }
        }
        drained
    }

    pub fn group_for_item(&mut self, item: Id) {
        let events = self.drain_history_until(|e| {
            if let Some(id) = e.item() {
                id == item
            } else {
                false
            }
        });

        if let Some(event) = SceneEvent::set(events) {
            self.history.push(event);
        }
    }

    fn group_moves_single(&mut self, last: SceneEvent) {
        let (sprite, mut start, finish) = if let SceneEvent::SpriteMove(id, from, to) = last {
            (id, from, to)
        } else {
            return;
        };

        let mut events = Vec::new();

        self.consume_history_until(|e| match e {
            SceneEvent::SpriteMove(id, from, _) => {
                if *id == sprite {
                    start = *from;
                    true
                } else {
                    false
                }
            }
            SceneEvent::SpriteNew(s, _) => {
                if s.id == sprite {
                    events.push(e.clone());
                    true
                } else {
                    false
                }
            }
            _ => false,
        });

        events.push(SceneEvent::SpriteMove(sprite, start, finish));
        if let Some(event) = SceneEvent::set(events) {
            self.history.push(event);
        }
    }

    pub fn group_moves_drawing(&mut self, last: SceneEvent) {
        let SceneEvent::SpriteDrawingPoint(drawing, _) = last else {
            return;
        };

        // Just remove all sprite drawing point events. To undo a drawing, we
        // remove the drawing and sprite.
        let mut events = Vec::new();
        self.consume_history_until(|e| match e {
            SceneEvent::SpriteDrawingPoint(id, ..) => *id == drawing,
            SceneEvent::SpriteNew(sprite, _) => {
                if let Some(id) = sprite.visual.drawing() {
                    if id == drawing {
                        events.push(e.clone());
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            SceneEvent::SpriteDrawingStart(id, _) => {
                if *id == drawing {
                    events.push(e.clone());
                    true
                } else {
                    false
                }
            }
            SceneEvent::EventSet(es) => {
                let mut ret = false;
                for e in es {
                    if let SceneEvent::SpriteDrawingStart(id, _) = e {
                        if *id == drawing {
                            events.push(e.clone());
                            ret = true;
                        }
                    }
                }
                ret
            }
            _ => false,
        });

        if let Some(event) = SceneEvent::set(events) {
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

        if let Some(event) = SceneEvent::set(moves.into_values().collect::<Vec<SceneEvent>>()) {
            self.history.push(event);
        }
    }

    pub fn end_move_group(&mut self) {
        let opt = self.history.pop();
        if let Some(event) = opt {
            match event {
                SceneEvent::SpriteMove(..) => self.group_moves_single(event),
                SceneEvent::EventSet(..) => self.group_moves_set(event),
                SceneEvent::SpriteDrawingPoint(..) => self.group_moves_drawing(event),
                _ => self.history.push(event),
            };
        }
    }

    pub fn change_scene(&mut self, scene_key: String) -> bool {
        self.issue_message(ClientEvent::SceneChange(scene_key));
        self.client.is_some()
    }

    pub fn erase_item(&mut self, id_to_erase: Id) {
        let predicate = |e: &SceneEvent| {
            if let Some(id) = e.item() {
                id != id_to_erase
            } else {
                true
            }
        };
        self.history.retain(predicate);
        self.redo_history.retain(predicate);
    }

    pub fn disconnect(&mut self) {
        if let Some(client) = &mut self.client {
            client.disconnect();
        }
    }

    pub fn reply_to_health_check(&mut self) {
        crate::bridge::log!("PING!");
        self.issue_message(ClientEvent::Ping);
    }
}

#[cfg(test)]
mod test {
    use scene::{comms::SceneEvent, Point};

    use crate::interactor::Interactor;

    #[test]
    fn test_group_drawing_events() {
        let mut int = Interactor::new(None);

        int.start_draw(
            Point::ORIGIN,
            false,
            false,
            Default::default(),
            crate::viewport::DrawTool::Freehand,
        );
        int.drag(Point::same(0.5), false);
        int.drag(Point::same(1.0), false);
        int.drag(Point::same(1.5), false);

        // new drawing, new sprite, 3 points
        assert!(int.history.history.len() == 5);

        int.release(false, false);

        // events should all be grouped to undo as one
        assert!(int.history.history.len() == 1);
        let event = int.history.history.first().unwrap();
        let SceneEvent::EventSet(events) = event else {
            panic!("Should have a SpriteDrawingStart and a SpriteNew event.");
        };
        assert_eq!(events.len(), 2);
        assert!(events
            .iter()
            .any(|e| matches!(e, SceneEvent::SpriteDrawingStart(..))));
        assert!(events
            .iter()
            .any(|e| matches!(e, SceneEvent::SpriteNew(..))));
    }
}
