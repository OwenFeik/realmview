use std::sync::atomic::{AtomicI64, Ordering};

use bincode::serialize;
use scene::{
    comms::{ClientEvent, ClientMessage, SceneEvent, SceneEventAck, ServerEvent},
    Id, Layer, Rect, Scene, ScenePoint, Sprite,
};

use crate::client::Client;

#[derive(Clone, Copy, Debug)]
enum HeldObject {
    Anchor(Id, i32, i32),
    Marquee(ScenePoint),
    None,
    Selection(ScenePoint),
    Sprite(Id, ScenePoint),
}

impl HeldObject {
    // Distance in scene units from which anchor points (corners, edges) of the
    // sprite can be dragged.
    const ANCHOR_RADIUS: f32 = 0.2;

    fn is_none(&self) -> bool {
        matches!(self, HeldObject::None)
    }

    fn grab_sprite_anchor(sprite: &Sprite, at: ScenePoint) -> Option<Self> {
        let Rect { x, y, w, h } = sprite.rect;

        // Anchor size is 0.2 tiles or one fifth of the smallest dimension of
        // the sprite. This is to allow sprites that are ANCHOR_RADIUS or
        // smaller to nonetheless be grabbed.
        let mut closest_dist = Self::ANCHOR_RADIUS.min(w.abs().min(h.abs()) / 5.0);
        let mut closest: (i32, i32) = (2, 2);
        for dx in -1..2 {
            for dy in -1..2 {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let anchor_x = x + (w / 2.0) * (dx + 1) as f32;
                let anchor_y = y + (h / 2.0) * (dy + 1) as f32;

                let delta_x = anchor_x - at.x;
                let delta_y = anchor_y - at.y;

                let dist = (delta_x.powi(2) + delta_y.powi(2)).sqrt();
                if dist <= closest_dist {
                    closest = (dx, dy);
                    closest_dist = dist;
                }
            }
        }

        if closest != (2, 2) {
            Some(Self::Anchor(sprite.local_id, closest.0, closest.1))
        } else {
            None
        }
    }

    fn grab_sprite(sprite: &Sprite, at: ScenePoint) -> Self {
        Self::grab_sprite_anchor(sprite, at)
            .unwrap_or_else(|| Self::Sprite(sprite.local_id, at - sprite.rect.top_left()))
    }
}

pub struct Interactor {
    changed: bool,
    client: Option<Client>,
    holding: HeldObject,
    issued_events: Vec<ClientMessage>,
    scene: Scene,
    selected_sprites: Option<Vec<Id>>,
    selection_marquee: Option<Rect>,
}

impl Interactor {
    pub fn new(client: Option<Client>) -> Self {
        Interactor {
            changed: false,
            client,
            holding: HeldObject::None,
            issued_events: vec![],
            scene: Scene::new(),
            selected_sprites: None,
            selection_marquee: None,
        }
    }

    fn change(&mut self) {
        self.changed = true;
    }

    pub fn handle_change(&mut self) -> bool {
        let ret = self.changed;
        self.changed = false;
        ret
    }

    pub fn process_server_events(&mut self) {
        if let Some(client) = &self.client {
            let mut events = client.events();
            while let Some(event) = events.pop() {
                self.process_server_event(event);
                self.change();
            }
        }
    }

    fn approve_event(&mut self, id: Id) {
        self.issued_events.retain(|c| c.id != id);
    }

    fn unwind_event(&mut self, id: Id) {
        if let Some(i) = self.issued_events.iter().position(|c| c.id == id) {
            if let ClientEvent::SceneChange(e) = self.issued_events.remove(i).event {
                self.scene.unwind_event(e);
            }
        }
    }

    fn process_scene_ack(&mut self, id: Id, ack: SceneEventAck) {
        match ack {
            SceneEventAck::Rejection => self.unwind_event(id),
            _ => {
                self.scene.apply_ack(&ack);
                self.approve_event(id);
            }
        };
    }

    fn process_server_event(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::Ack(id, None) => self.approve_event(id),
            ServerEvent::Ack(id, Some(ack)) => self.process_scene_ack(id, ack),
            ServerEvent::SceneChange(scene) => self.replace_scene(scene),
            ServerEvent::SceneUpdate(scene_event) => {
                self.scene.apply_event(scene_event);
            }
        }
    }

    fn client_event(&mut self, scene_event: SceneEvent) {
        static EVENT_ID: AtomicI64 = AtomicI64::new(1);

        // nop unless we actually have a connection.
        if let Some(client) = &self.client {
            let message = ClientMessage {
                id: EVENT_ID.fetch_add(1, Ordering::Relaxed),
                event: ClientEvent::SceneChange(scene_event),
            };
            client.send_message(&message);
            self.issued_events.push(message);
        }
    }

    fn client_option(&mut self, event_option: Option<SceneEvent>) {
        if let Some(scene_event) = event_option {
            self.client_event(scene_event);
        }
    }

    fn held_id(&self) -> Option<Id> {
        match self.holding {
            HeldObject::Sprite(id, _) => Some(id),
            HeldObject::Anchor(id, _, _) => Some(id),
            _ => None,
        }
    }

    fn held_sprite(&mut self) -> Option<&mut Sprite> {
        match self.held_id() {
            Some(id) => self.scene.sprite(id),
            None => None,
        }
    }

    pub fn grab(&mut self, at: ScenePoint) {
        self.holding = match self.scene.sprite_at(at) {
            Some(s) => {
                if self.selected_sprites.is_some()
                    && self
                        .selected_sprites
                        .as_ref()
                        .unwrap()
                        .contains(&s.local_id)
                {
                    HeldObject::Selection(at)
                } else {
                    HeldObject::grab_sprite(s, at)
                }
            }
            None => {
                self.selected_sprites = None;
                HeldObject::Marquee(at)
            }
        };
        self.change();
    }

    fn update_held_sprite(&mut self, at: ScenePoint) {
        let holding = self.holding;
        let sprite = if let Some(s) = self.held_sprite() {
            s
        } else {
            return;
        };

        let opt = match holding {
            HeldObject::Sprite(_, offset) => sprite.set_pos(at - offset),
            HeldObject::Anchor(_, dx, dy) => {
                let old_rect = sprite.rect;

                let ScenePoint {
                    x: delta_x,
                    y: delta_y,
                } = at - sprite.anchor_point(dx, dy);
                let x = sprite.rect.x + (if dx == -1 { delta_x } else { 0.0 });
                let y = sprite.rect.y + (if dy == -1 { delta_y } else { 0.0 });
                let w = delta_x * (dx as f32) + sprite.rect.w;
                let h = delta_y * (dy as f32) + sprite.rect.h;

                sprite.set_rect(Rect { x, y, w, h });
                sprite
                    .canonical_id
                    .map(|id| SceneEvent::SpriteMove(id, old_rect, sprite.rect))
            }
            _ => return, // Other types aren't sprite-related
        };
        self.client_option(opt);
        self.change();
    }

    fn drag_selection(&mut self, to: ScenePoint) {
        let delta = if let HeldObject::Selection(from) = self.holding {
            to - from
        } else {
            return;
        };

        if let Some(ids) = self.selected_sprites.clone() {
            let opts = ids
                .iter()
                .map(|id| {
                    if let Some(s) = self.scene.sprite(*id) {
                        s.move_by(delta)
                    } else {
                        None
                    }
                })
                .collect::<Vec<Option<SceneEvent>>>();

            for opt in opts {
                self.client_option(opt);
            }

            self.change();
        }

        self.holding = HeldObject::Selection(to);
    }

    pub fn drag(&mut self, at: ScenePoint) {
        match self.holding {
            HeldObject::Marquee(from) => {
                self.selection_marquee = Some(from.rect(at));
                self.change();
            }
            HeldObject::None => {}
            HeldObject::Selection(_) => self.drag_selection(at),
            HeldObject::Sprite(_, _) | HeldObject::Anchor(_, _, _) => self.update_held_sprite(at),
        };
    }

    fn release_held_sprite(&mut self, id: Id, snap_to_grid: bool) {
        if let Some(s) = self.scene.sprite(id) {
            let opt = if snap_to_grid {
                s.snap_to_grid()
            } else {
                s.enforce_min_size()
            };
            self.client_option(opt);
            self.change();
        };
    }

    pub fn release(&mut self, alt: bool) {
        match self.holding {
            HeldObject::Marquee(_) => {
                if let Some(region) = self.selection_marquee {
                    self.selected_sprites = Some(self.scene.sprites_in(region, alt));
                }
                self.selection_marquee = None;
                self.change();
            }
            HeldObject::None => {}
            HeldObject::Selection(_) => {
                if let Some(ids) = self.selected_sprites.clone() {
                    for id in ids {
                        self.release_held_sprite(id, !alt);
                    }
                }
            }
            HeldObject::Sprite(id, _) | HeldObject::Anchor(id, _, _) => {
                self.release_held_sprite(id, !alt)
            }
        };

        self.holding = HeldObject::None;
    }

    #[must_use]
    pub fn dimensions(&self) -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            w: self.scene.w as f32,
            h: self.scene.h as f32,
        }
    }

    #[must_use]
    pub fn export(&self) -> Vec<u8> {
        match serialize(&self.scene) {
            Ok(v) => v,
            Err(_) => vec![],
        }
    }

    #[must_use]
    pub fn layers(&self) -> &[Layer] {
        &self.scene.layers
    }

    pub fn new_scene(&mut self, id: Id) {
        if self.scene.id.is_some() {
            self.scene = Scene::new();
            if id != 0 {
                self.scene.project = Some(id);
            }
            self.change();
        }
    }

    pub fn replace_scene(&mut self, mut new: Scene) {
        new.refresh_local_ids();
        self.scene = new;
        self.change();
    }

    pub fn new_layer(&mut self) {
        let z = self
            .scene
            .layers
            .get(0)
            .map(|l| (l.z + 1).max(1))
            .unwrap_or(1);
        let opt = self.scene.add_layer(Layer::new("Untitled", z));
        self.client_option(opt);
    }

    pub fn remove_layer(&mut self, layer: Id) {
        let opt = self.scene.remove_layer(layer);
        self.client_option(opt);
        self.change();
    }

    pub fn rename_layer(&mut self, layer: Id, title: String) {
        let opt = self.scene.rename_layer(layer, title);
        self.client_option(opt);
    }

    pub fn set_layer_visible(&mut self, layer: Id, visible: bool) {
        if let Some(l) = self.scene.layer(layer) {
            let opt = l.set_visible(visible);
            self.client_option(opt);
            self.change();
        }
    }

    pub fn set_layer_locked(&mut self, layer: Id, locked: bool) {
        if let Some(l) = self.scene.layer(layer) {
            let opt = l.set_locked(locked);
            self.client_option(opt);
        }
    }

    pub fn move_layer(&mut self, layer: Id, up: bool) {
        let opt = self.scene.move_layer(layer, up);
        self.client_option(opt);
        self.change();
    }

    pub fn new_sprite(&mut self, texture: Id, layer: Id) {
        let opt = self.scene.add_sprite(Sprite::new(texture), layer);
        self.client_option(opt);
        self.change();
    }

    pub fn selections(&mut self) -> Vec<Rect> {
        let mut selections = vec![];

        if let Some(ids) = &self.selected_sprites {
            for id in ids {
                if let Some(s) = self.scene.sprite(*id) {
                    selections.push(s.rect);
                }
            }
        }

        if let Some(sprite) = self.held_sprite() {
            selections.push(sprite.rect);
        }

        if let Some(rect) = self.selection_marquee {
            selections.push(rect);
        }
        selections
    }
}