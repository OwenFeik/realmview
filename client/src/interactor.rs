use std::{
    collections::HashMap,
    sync::atomic::{AtomicI64, Ordering},
};

use bincode::serialize;
use scene::{
    comms::{ClientEvent, ClientMessage, SceneEvent, ServerEvent},
    perms::Perms,
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

    fn is_sprite(&self) -> bool {
        matches!(
            self,
            HeldObject::Sprite(..) | HeldObject::Anchor(..) | HeldObject::Selection(..)
        )
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
            Some(Self::Anchor(sprite.id, closest.0, closest.1))
        } else {
            None
        }
    }

    fn grab_sprite(sprite: &Sprite, at: ScenePoint) -> Self {
        Self::grab_sprite_anchor(sprite, at)
            .unwrap_or_else(|| Self::Sprite(sprite.id, at - sprite.rect.top_left()))
    }
}

pub struct Interactor {
    changed: bool,
    client: Option<Client>,
    holding: HeldObject,
    history: Vec<SceneEvent>,
    redo_history: Vec<Option<SceneEvent>>,
    issued_events: Vec<ClientMessage>,
    layers_changed: bool,
    perms: Perms,
    scene: Scene,
    selected_sprites: Option<Vec<Id>>,
    selection_marquee: Option<Rect>,
    user: Id,
}

impl Interactor {
    pub const SELECTION_ID: Id = -1;

    pub fn new(client: Option<Client>) -> Self {
        Interactor {
            changed: true,
            client,
            holding: HeldObject::None,
            history: vec![],
            redo_history: vec![],
            issued_events: vec![],
            layers_changed: true,
            perms: Perms::new(),
            scene: Scene::new(),
            selected_sprites: None,
            selection_marquee: None,
            user: scene::perms::CANONICAL_UPDATER,
        }
    }

    fn change(&mut self) {
        self.changed = true;
    }

    fn change_if(&mut self, changed: bool) {
        if changed {
            self.change();
        }
    }

    pub fn handle_change(&mut self) -> bool {
        let ret = self.changed;
        self.changed = false;
        ret
    }

    fn layer_change(&mut self) {
        self.layers_changed = true;
    }

    fn layer_change_if(&mut self, changed: bool) {
        if changed {
            self.layer_change();
        }
    }

    pub fn handle_layer_change(&mut self) -> bool {
        let ret = self.layers_changed;
        self.layers_changed = false;
        ret
    }

    fn both_change(&mut self) {
        self.change();
        self.layer_change();
    }

    pub fn process_server_events(&mut self) {
        if let Some(client) = &self.client {
            for event in client.events() {
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
            if let ClientEvent::SceneUpdate(e) = self.issued_events.remove(i).event {
                // If we got rejected while dragging a sprite, release that
                // sprite to prevent visual jittering and allow the position to
                // reset.
                if self.held_id() == e.item() {
                    self.holding = HeldObject::None;
                }

                self.layer_change_if(e.is_layer());
                self.change();
                self.scene.unwind_event(e);
            }
        }
    }

    fn process_server_event(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::Approval(id) => self.approve_event(id),
            ServerEvent::Rejection(id) => self.unwind_event(id),
            ServerEvent::PermsChange(perms) => self.replace_perms(perms),
            ServerEvent::PermsUpdate(perms_event) => {
                self.perms
                    .handle_event(scene::perms::CANONICAL_UPDATER, perms_event);
            }
            ServerEvent::SceneChange(scene) => self.replace_scene(scene),
            ServerEvent::SceneUpdate(scene_event) => {
                self.layer_change_if(scene_event.is_layer());
                if self.scene.apply_event(scene_event) {
                    self.change();
                }
            }
            ServerEvent::UserId(id) => {
                self.user = id;
            }
        }
    }

    fn issue_client_event(&mut self, scene_event: SceneEvent) {
        static EVENT_ID: AtomicI64 = AtomicI64::new(1);

        // Queue event to be sent to server
        if let Some(client) = &self.client {
            let message = ClientMessage {
                id: EVENT_ID.fetch_add(1, Ordering::Relaxed),
                event: ClientEvent::SceneUpdate(scene_event),
            };
            client.send_message(&message);
            self.issued_events.push(message);
        }
    }

    fn scene_event(&mut self, event: SceneEvent) {
        if self
            .perms
            .permitted(self.user, &event, self.scene.event_layer(&event))
        {
            self.issue_client_event(event.clone());

            // When adding a new entry to the history, all undone events are lost.
            self.redo_history.clear();
            self.history.push(event);
        } else {
            self.scene.unwind_event(event);
        }
    }

    fn scene_option(&mut self, event_option: Option<SceneEvent>) {
        if let Some(event) = event_option {
            self.scene_event(event);
        }
    }

    fn start_move_group(&mut self) {
        self.history.push(SceneEvent::Dummy);
    }

    fn group_moves_single(&mut self, last: SceneEvent) {
        let (sprite, mut start, finish) = if let SceneEvent::SpriteMove(id, from, to) = last {
            (id, from, to)
        } else {
            return;
        };

        while let Some(e) = self.history.pop() {
            if let SceneEvent::SpriteMove(id, from, _) = e {
                if id == sprite {
                    start = from;
                    continue;
                }
            }

            if !matches!(e, SceneEvent::Dummy) {
                self.history.push(e);
            }
            break;
        }

        self.history
            .push(SceneEvent::SpriteMove(sprite, start, finish));
    }

    fn group_moves_set(&mut self, last: SceneEvent) {
        self.history.push(last);
        let mut moves = HashMap::new();

        while let Some(e) = self.history.pop() {
            if let SceneEvent::EventSet(v) = e {
                for event in v {
                    if let SceneEvent::SpriteMove(id, from, _) = event {
                        if let Some(SceneEvent::SpriteMove(_, start, _)) = moves.get_mut(&id) {
                            *start = from;
                        } else {
                            moves.insert(id, event);
                        }
                    }
                }
                continue;
            }

            if !matches!(e, SceneEvent::Dummy) {
                self.history.push(e);
            }
            break;
        }

        self.history.push(SceneEvent::EventSet(
            moves.into_values().collect::<Vec<SceneEvent>>(),
        ));
    }

    fn end_move_group(&mut self) {
        let opt = self.history.pop();
        if let Some(event) = opt {
            match event {
                SceneEvent::SpriteMove(..) => self.group_moves_single(event),
                SceneEvent::EventSet(..) => self.group_moves_set(event),
                _ => self.history.push(event),
            };
        }
    }

    pub fn undo(&mut self) {
        if let Some(event) = self.history.pop() {
            if matches!(event, SceneEvent::Dummy) {
                self.undo();
                return;
            }

            let opt = self.scene.unwind_event(event);
            if let Some(event) = &opt {
                self.issue_client_event(event.clone());
                self.change();
            }
            self.redo_history.push(opt);
        }
    }

    pub fn redo(&mut self) {
        if let Some(Some(event)) = self.redo_history.pop() {
            if let Some(event) = self.scene.unwind_event(event) {
                self.issue_client_event(event.clone());
                self.history.push(event);
                self.change();
            }
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

    /// Apply a closure to each selected sprite, issuing the resulting vector
    /// of events as a single EventSet event.
    fn selection_effect<F: Fn(&mut Sprite) -> Option<SceneEvent>>(&mut self, effect: F) {
        if let Some(ids) = &self.selected_sprites {
            let events = ids
                .iter()
                .filter_map(|id| {
                    if let Some(s) = self.scene.sprite(*id) {
                        effect(s)
                    } else {
                        None
                    }
                })
                .collect::<Vec<SceneEvent>>();

            if !events.is_empty() {
                self.scene_event(SceneEvent::EventSet(events));
                self.change();
            }
        }
    }

    pub fn grab(&mut self, at: ScenePoint, ctrl: bool) {
        self.holding = match self.scene.sprite_at(at) {
            Some(s) => {
                if let Some(selected) = &mut self.selected_sprites {
                    let already = selected.contains(&s.id);
                    if already || ctrl {
                        if !already && ctrl {
                            selected.push(s.id);
                        }
                        HeldObject::Selection(at)
                    } else {
                        selected.clear();
                        selected.push(s.id);
                        HeldObject::grab_sprite(s, at)
                    }
                } else {
                    self.selected_sprites = Some(vec![s.id]);
                    HeldObject::grab_sprite(s, at)
                }
            }
            None => HeldObject::Marquee(at),
        };

        if self.holding.is_sprite() {
            self.start_move_group();
        }

        self.change();
    }

    fn update_held_sprite(&mut self, at: ScenePoint) {
        let holding = self.holding;
        let sprite = if let Some(s) = self.held_sprite() {
            s
        } else {
            return;
        };

        let event = match holding {
            HeldObject::Sprite(_, offset) => sprite.set_pos(at - offset),
            HeldObject::Anchor(_, dx, dy) => {
                let ScenePoint {
                    x: delta_x,
                    y: delta_y,
                } = at - sprite.anchor_point(dx, dy);
                let x = sprite.rect.x + (if dx == -1 { delta_x } else { 0.0 });
                let y = sprite.rect.y + (if dy == -1 { delta_y } else { 0.0 });
                let w = delta_x * (dx as f32) + sprite.rect.w;
                let h = delta_y * (dy as f32) + sprite.rect.h;

                sprite.set_rect(Rect { x, y, w, h })
            }
            _ => return, // Other types aren't sprite-related
        };
        self.scene_event(event);
        self.change();
    }

    fn drag_selection(&mut self, to: ScenePoint) {
        let delta = if let HeldObject::Selection(from) = self.holding {
            to - from
        } else {
            return;
        };

        self.selection_effect(|s| Some(s.move_by(delta)));
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

    pub fn sprite_at(&self, at: ScenePoint) -> Option<Id> {
        if let Some(id) = self.scene.sprite_at_ref(at).map(|s| s.id) {
            if let Some(ids) = &self.selected_sprites {
                if ids.contains(&id) {
                    return Some(Self::SELECTION_ID);
                }
            }
            return Some(id);
        }
        None
    }

    fn release_sprite(sprite: &mut Sprite, snap_to_grid: bool) -> Option<SceneEvent> {
        if snap_to_grid {
            Some(sprite.snap_to_grid())
        } else {
            sprite.enforce_min_size()
        }
    }

    fn release_held_sprite(&mut self, id: Id, snap_to_grid: bool) {
        if let Some(s) = self.scene.sprite(id) {
            let opt = Self::release_sprite(s, snap_to_grid);
            self.scene_option(opt);
            self.change();
        };
    }

    fn release_selection(&mut self, snap_to_grid: bool) {
        self.selection_effect(|s| Self::release_sprite(s, snap_to_grid));
    }

    pub fn release(&mut self, alt: bool, ctrl: bool) {
        match self.holding {
            HeldObject::Marquee(_) => {
                if !ctrl {
                    self.selected_sprites = None;
                }

                if let Some(region) = self.selection_marquee {
                    let mut selection = self.scene.sprites_in(region, alt);
                    if ctrl && self.selected_sprites.is_some() {
                        self.selected_sprites
                            .as_mut()
                            .unwrap()
                            .append(&mut selection);
                    } else {
                        self.selected_sprites = Some(selection);
                    }
                }
                self.selection_marquee = None;
                self.change();
            }
            HeldObject::None => {}
            HeldObject::Selection(_) => self.release_selection(!alt),
            HeldObject::Sprite(id, _) | HeldObject::Anchor(id, _, _) => {
                self.release_held_sprite(id, !alt)
            }
        };

        if self.holding.is_sprite() {
            self.end_move_group();
        }

        self.holding = HeldObject::None;
    }

    #[must_use]
    pub fn layers(&self) -> &[Layer] {
        &self.scene.layers
    }

    #[must_use]
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

    pub fn new_scene(&mut self, id: Id) {
        if self.scene.id.is_some() {
            self.scene = Scene::new();
            if id != 0 {
                self.scene.project = Some(id);
            }
            self.change();
        }
    }

    fn replace_perms(&mut self, new: Perms) {
        self.perms = new;
    }

    pub fn replace_scene(&mut self, new: Scene) {
        self.scene = new;
        self.both_change();
    }

    pub fn new_layer(&mut self) {
        let z = self
            .scene
            .layers
            .get(0)
            .map(|l| (l.z + 1).max(1))
            .unwrap_or(1);
        let opt = self.scene.new_layer("Untitled", z);
        self.scene_option(opt);
        self.layer_change();
    }

    pub fn remove_layer(&mut self, layer: Id) {
        let opt = self.scene.remove_layer(layer);
        self.scene_option(opt);
        self.both_change();
    }

    pub fn rename_layer(&mut self, layer: Id, title: String) {
        let opt = self.scene.rename_layer(layer, title);
        self.scene_option(opt);
        self.layer_change();
    }

    pub fn set_layer_visible(&mut self, layer: Id, visible: bool) {
        if let Some(l) = self.scene.layer(layer) {
            let opt = l.set_visible(visible);
            let changed = !l.sprites.is_empty();
            self.change_if(changed);
            self.scene_option(opt);
        }
    }

    pub fn set_layer_locked(&mut self, layer: Id, locked: bool) {
        if let Some(l) = self.scene.layer(layer) {
            let opt = l.set_locked(locked);
            self.scene_option(opt);
        }
    }

    pub fn move_layer(&mut self, layer: Id, up: bool) {
        let opt = self.scene.move_layer(layer, up);
        self.scene_option(opt);
        self.both_change();
    }

    pub fn new_sprite(&mut self, texture: Id, layer: Id) {
        let opt = self.scene.new_sprite(texture, layer);
        self.scene_option(opt);
        self.change();
    }

    pub fn remove_sprite(&mut self, sprite: Id) {
        if sprite == Self::SELECTION_ID {
            if let Some(ids) = &self.selected_sprites {
                let event = self.scene.remove_sprites(ids);
                self.scene_event(event);
                self.change();
            }
        } else {
            let opt = self.scene.remove_sprite(sprite);
            self.scene_option(opt);
            self.change();
        }
    }

    pub fn sprite_layer(&mut self, sprite: Id, layer: Id) {
        if sprite == Self::SELECTION_ID {
            if let Some(ids) = &self.selected_sprites {
                let event = self.scene.sprites_layer(ids, layer);
                self.scene_event(event);
                self.change();
            }
        } else {
            let opt = self.scene.sprite_layer(sprite, layer);
            self.scene_option(opt);
            self.change();
        }
    }

    pub fn sprite_rect(&mut self, sprite: Id, rect: Rect) {
        let opt = self.scene.sprite(sprite).map(|s| s.set_rect(rect));
        self.scene_option(opt);
    }
}
