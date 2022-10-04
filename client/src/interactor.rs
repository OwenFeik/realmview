use std::{
    collections::HashMap,
    sync::atomic::{AtomicI64, Ordering},
};

use bincode::serialize;

use crate::scene::{
    comms::{ClientEvent, ClientMessage, SceneEvent, ServerEvent},
    perms::Perms,
    Colour, Dimension, Id, Layer, Point, Rect, Scene, Sprite, SpriteDrawing, SpriteShape,
    SpriteVisual,
};
use crate::{bridge::Cursor, client::Client};

pub struct Changes {
    // A change to a layer locked status, title, visibility, etc that will
    // require the layers list to be updated.
    layer: bool,

    // A change to a sprite that will require a re-render
    sprite: bool,

    // A change to the selected sprite that will require the sprite menu to be
    // updated.
    selected: bool,
}

impl Changes {
    fn new() -> Self {
        Changes {
            layer: true,
            sprite: true,
            selected: true,
        }
    }

    fn all_change(&mut self) {
        self.layer = true;
        self.sprite = true;
        self.selected = true;
    }

    fn all_change_if(&mut self, changed: bool) {
        self.layer_change_if(changed);
        self.sprite_change_if(changed);
        self.selected_change_if(changed);
    }

    fn layer_change(&mut self) {
        self.layer = true;
    }

    fn layer_change_if(&mut self, changed: bool) {
        self.layer = self.layer || changed;
    }

    pub fn handle_layer_change(&mut self) -> bool {
        let ret = self.layer;
        self.layer = false;
        ret
    }

    fn sprite_change(&mut self) {
        self.sprite = true;
    }

    fn sprite_change_if(&mut self, changed: bool) {
        self.sprite = self.sprite || changed;
    }

    pub fn handle_sprite_change(&mut self) -> bool {
        let ret = self.sprite;
        self.sprite = false;
        ret
    }

    fn selected_change(&mut self) {
        self.selected = true;
    }

    fn selected_change_if(&mut self, changed: bool) {
        self.selected = self.selected || changed;
    }

    pub fn handle_selected_change(&mut self) -> bool {
        let ret = self.selected;
        self.selected = false;
        ret
    }

    fn sprite_selected_change(&mut self) {
        self.sprite = true;
        self.selected = true;
    }
}

#[derive(Debug, Default, serde_derive::Deserialize, serde_derive::Serialize)]
#[serde(default)]
pub struct SpriteDetails {
    pub id: Id,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub w: Option<f32>,
    pub h: Option<f32>,
    pub shape: Option<SpriteShape>,
    pub stroke: Option<f32>,
    pub colour: Option<Colour>,
    pub texture: Option<Id>,
    pub drawing_type: Option<scene::SpriteDrawingType>,
    pub cap_start: Option<scene::SpriteCap>,
    pub cap_end: Option<scene::SpriteCap>,
}

impl SpriteDetails {
    fn from(id: Id, sprite: &Sprite) -> Self {
        SpriteDetails {
            id,
            x: Some(sprite.rect.x),
            y: Some(sprite.rect.y),
            w: Some(sprite.rect.w),
            h: Some(sprite.rect.h),
            shape: sprite.visual.shape(),
            stroke: sprite.visual.stroke(),
            colour: sprite.visual.colour(),
            texture: sprite.visual.texture(),
            drawing_type: sprite.visual.drawing().map(|d| d.drawing_type),
            cap_start: sprite.visual.cap_start(),
            cap_end: sprite.visual.cap_end(),
        }
    }

    fn update_from(&mut self, other: &Self) {
        self.id = other.id;

        if other.x.is_some() {
            self.x = other.x;
        }

        if other.y.is_some() {
            self.y = other.y;
        }

        if other.w.is_some() {
            self.w = other.w;
        }

        if other.h.is_some() {
            self.h = other.h;
        }

        // Special case for shape because setting to no shape is meaningful
        self.shape = other.shape;

        if other.stroke.is_some() {
            self.stroke = other.stroke;
        }

        if other.colour.is_some() {
            self.colour = other.colour;
        }

        if other.texture.is_some() {
            self.texture = other.texture;
        }

        if other.drawing_type.is_some() {
            self.drawing_type = other.drawing_type;
        }

        if other.cap_start.is_some() {
            self.cap_start = other.cap_start;
        }

        if other.cap_end.is_some() {
            self.cap_end = other.cap_end;
        }
    }

    fn colour(&self) -> Colour {
        self.colour.unwrap_or(Sprite::DEFAULT_COLOUR)
    }

    fn stroke(&self) -> f32 {
        self.stroke.unwrap_or(Sprite::DEFAULT_STROKE)
    }

    fn drawing_type(&self) -> scene::SpriteDrawingType {
        self.drawing_type
            .unwrap_or(scene::SpriteDrawing::DEFAULT_TYPE)
    }

    fn cap_start(&self) -> scene::SpriteCap {
        self.cap_start.unwrap_or(scene::SpriteCap::DEFAULT_START)
    }

    fn cap_end(&self) -> scene::SpriteCap {
        self.cap_end.unwrap_or(scene::SpriteCap::DEFAULT_END)
    }

    fn common(&mut self, sprite: &Sprite) {
        if self.x != Some(sprite.rect.x) {
            self.x = None;
        }

        if self.y != Some(sprite.rect.y) {
            self.y = None;
        }

        if self.w != Some(sprite.rect.w) {
            self.w = None;
        }

        if self.h != Some(sprite.rect.h) {
            self.h = None;
        }

        if self.shape.is_some() && self.shape != sprite.visual.shape() {
            self.shape = None;
        }

        if self.stroke.is_some() && self.stroke != sprite.visual.stroke() {
            self.stroke = None;
        }

        if self.colour.is_some() && self.colour != sprite.visual.colour() {
            self.colour = None;
        }

        if self.texture.is_some() && self.texture != sprite.visual.texture() {
            self.texture = None;
        }

        if self.drawing_type.is_some()
            && self.drawing_type != sprite.visual.drawing().map(|d| d.drawing_type)
        {
            self.drawing_type = None;
        }

        if self.cap_start.is_some() && self.cap_start != sprite.visual.cap_start() {
            self.cap_start = None;
        }

        if self.cap_end.is_some() && self.cap_end != sprite.visual.cap_end() {
            self.cap_end = None;
        }
    }

    fn update_sprite(&self, sprite: &mut Sprite) -> Option<SceneEvent> {
        let mut events = vec![];
        if let Some(x) = self.x {
            events.push(sprite.set_dimension(Dimension::X, x));
        }

        if let Some(y) = self.y {
            events.push(sprite.set_dimension(Dimension::Y, y));
        }

        if let Some(w) = self.w {
            events.push(sprite.set_dimension(Dimension::W, w));
        }

        if let Some(h) = self.h {
            events.push(sprite.set_dimension(Dimension::H, h));
        }

        if let Some(shape) = self.shape {
            if let Some(event) = sprite.set_shape(shape) {
                events.push(event);
            }
        }

        if let Some(stroke) = self.stroke {
            if let Some(event) = sprite.set_stroke(stroke) {
                events.push(event);
            }
        }

        if let Some(c) = self.colour {
            if let Some(event) = sprite.set_colour(c) {
                events.push(event);
            }
        }

        if let Some(id) = self.texture {
            if let Some(event) = sprite.set_texture(id) {
                events.push(event);
            }
        }

        if let Some(drawing_type) = self.drawing_type {
            if let Some(event) = sprite.set_drawing_type(drawing_type) {
                events.push(event);
            }
        }

        if let Some(event) = sprite.set_caps(self.cap_start, self.cap_end) {
            events.push(event);
        }

        if events.is_empty() {
            None
        } else {
            Some(SceneEvent::EventSet(events))
        }
    }

    fn drawing(&self) -> SpriteDrawing {
        let mut drawing = SpriteDrawing::new();
        drawing.drawing_type = self.drawing_type();
        drawing.colour = self.colour();
        drawing.cap_start = self.cap_start();
        drawing.cap_end = self.cap_end();
        drawing.stroke = self.stroke();
        drawing
    }
}

#[derive(Debug, Default, serde_derive::Deserialize, serde_derive::Serialize)]
#[serde(default)]
pub struct SceneDetails {
    pub id: Option<Id>,
    pub title: Option<String>,
    pub w: Option<u32>,
    pub h: Option<u32>,
}

impl SceneDetails {
    fn from(scene: &Scene) -> Self {
        SceneDetails {
            id: scene.id,
            title: scene.title.clone(),
            w: Some(scene.w),
            h: Some(scene.h),
        }
    }

    fn update_scene(&self, scene: &mut Scene) {
        if self.title.is_some() {
            scene.title = self.title.clone();
        }

        if let Some(w) = self.w {
            scene.w = w;
        }

        if let Some(h) = self.h {
            scene.h = h;
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum HeldObject {
    Anchor(Id, i32, i32, Rect), // (sprite, dx, dy, starting_rect)
    Drawing(Id, bool),          // (sprite, ephemeral)
    Marquee(Point),
    None,
    Selection(Point),
    Sprite(Id, Point, Rect), // (sprite, delta, starting_rect)
}

impl HeldObject {
    // Distance in scene units from which anchor points (corners, edges) of the
    // sprite can be dragged.
    const ANCHOR_RADIUS: f32 = 0.2;

    fn held_id(&self) -> Option<Id> {
        match self {
            Self::Anchor(id, ..) | Self::Drawing(id, ..) | Self::Sprite(id, ..) => Some(*id),
            _ => None,
        }
    }

    fn is_none(&self) -> bool {
        matches!(self, HeldObject::None)
    }

    fn is_sprite(&self) -> bool {
        matches!(
            self,
            HeldObject::Anchor(..) | HeldObject::Selection(..) | HeldObject::Sprite(..)
        )
    }

    fn grab_sprite_anchor(sprite: &Sprite, at: Point) -> Option<Self> {
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
            Some(Self::Anchor(sprite.id, closest.0, closest.1, sprite.rect))
        } else {
            None
        }
    }

    fn grab_sprite(sprite: &Sprite, at: Point) -> Self {
        Self::grab_sprite_anchor(sprite, at)
            .unwrap_or_else(|| Self::Sprite(sprite.id, at - sprite.rect.top_left(), sprite.rect))
    }

    fn cursor(&self) -> Cursor {
        match self {
            Self::Anchor(_, dx, dy, Rect { w, h, .. }) => match (dx, dy) {
                (-1, -1) | (1, 1) => {
                    if w.signum() == h.signum() {
                        Cursor::NwseResize
                    } else {
                        Cursor::NeswResize
                    }
                }
                (-1, 1) | (1, -1) => {
                    if w.signum() == h.signum() {
                        Cursor::NeswResize
                    } else {
                        Cursor::NwseResize
                    }
                }
                (0, -1) | (0, 1) => Cursor::NsResize,
                (-1, 0) | (1, 0) => Cursor::EwResize,
                _ => Cursor::Move,
            },
            Self::Drawing(..) => Cursor::Crosshair,
            Self::Marquee(..) | Self::None => Cursor::Default,
            Self::Selection(..) | Self::Sprite(..) => Cursor::Move,
        }
    }
}

pub struct Interactor {
    pub changes: Changes,
    client: Option<Client>,
    draw_details: SpriteDetails,
    holding: HeldObject,
    history: Vec<SceneEvent>,
    redo_history: Vec<Option<SceneEvent>>,
    issued_events: Vec<ClientMessage>,
    perms: Perms,
    scene: Scene,
    selected_layer: Id,
    selected_sprites: Vec<Id>,
    selection_marquee: Option<Rect>,
    user: Id,
}

impl Interactor {
    /// This special ID will not belong to any sprite, and will instead be used
    /// to refer to all currently selected sprites.
    const SELECTION_ID: Id = -1;

    pub fn new(client: Option<Client>) -> Self {
        let scene = Scene::new();
        let selected_layer = scene.first_layer();
        Interactor {
            changes: Changes::new(),
            client,
            draw_details: SpriteDetails::default(),
            holding: HeldObject::None,
            history: vec![],
            redo_history: vec![],
            issued_events: vec![],
            perms: Perms::new(),
            scene,
            selected_layer,
            selected_sprites: vec![],
            selection_marquee: None,
            user: scene::perms::CANONICAL_UPDATER,
        }
    }

    pub fn process_server_events(&mut self) {
        if let Some(client) = &self.client {
            for event in client.events() {
                self.process_server_event(event);
                self.changes.sprite_change();
            }
        }
    }

    pub fn cursor(&self) -> Cursor {
        self.holding.cursor()
    }

    pub fn cursor_at(&self, at: Point, ctrl: bool) -> Cursor {
        if matches!(self.holding, HeldObject::None) {
            match self.grab_at(at, ctrl).0 {
                HeldObject::Sprite(..) => Cursor::Pointer,
                h => h.cursor(),
            }
        } else {
            self.cursor()
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

                self.changes.layer_change_if(e.is_layer());
                self.changes.sprite_selected_change();
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
                self.changes.layer_change_if(scene_event.is_layer());
                self.scene.apply_event(scene_event);
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

            self.changes.layer_change_if(event.is_layer());
            self.changes.sprite_change_if(event.is_sprite());
            if let Some(id) = event.item() {
                self.changes.selected_change_if(self.is_selected(id));
            }

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

    fn end_move_group(&mut self) {
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

    pub fn undo(&mut self) {
        if let Some(event) = self.history.pop() {
            if matches!(event, SceneEvent::Dummy) {
                self.undo();
                return;
            }

            let opt = self.scene.unwind_event(event);
            if let Some(event) = &opt {
                let layers_changed = event.is_layer();
                self.issue_client_event(event.clone());
                self.changes.layer_change_if(layers_changed);
                self.changes.sprite_selected_change();
            }
            self.redo_history.push(opt);
        }
    }

    pub fn redo(&mut self) {
        if let Some(Some(event)) = self.redo_history.pop() {
            if let Some(event) = self.scene.unwind_event(event) {
                let layers_changed = event.is_layer();
                self.issue_client_event(event.clone());
                self.history.push(event);
                self.changes.layer_change_if(layers_changed);
                self.changes.sprite_selected_change();
            }
        }
    }

    fn held_id(&self) -> Option<Id> {
        match self.holding {
            HeldObject::Sprite(id, ..) => Some(id),
            HeldObject::Anchor(id, ..) => Some(id),
            _ => None,
        }
    }

    fn held_sprite(&mut self) -> Option<&mut Sprite> {
        match self.held_id() {
            Some(id) => self.scene.sprite(id),
            None => None,
        }
    }

    pub fn has_selection(&self) -> bool {
        !self.selected_sprites.is_empty()
    }

    fn is_selected(&self, id: Id) -> bool {
        self.selected_sprites.contains(&id)
    }

    fn single_selected(&self) -> bool {
        self.selected_sprites.len() == 1
    }

    pub fn clear_selection(&mut self) {
        self.selected_sprites.clear();
        self.changes.sprite_selected_change();
    }

    fn clear_held_selection(&mut self) {
        self.holding = HeldObject::None;
        self.clear_selection();
    }

    fn select(&mut self, id: Id) {
        if !self.is_selected(id) {
            self.selected_sprites.push(id);
            self.changes.sprite_selected_change();
        }
    }

    fn select_multiple(&mut self, ids: &mut Vec<Id>) {
        let mut ids = ids.drain_filter(|&mut id| !self.is_selected(id)).collect();
        self.selected_sprites.append(&mut ids);
    }

    pub fn select_all(&mut self) {
        if let Some(l) = self.scene.layer(self.selected_layer) {
            self.selected_sprites = l.sprites.iter().map(|s| s.id).collect();
            self.changes.sprite_selected_change();
        }
    }

    /// Apply a closure to each selected sprite, issuing the resulting vector
    /// of events as a single EventSet event.
    fn selection_effect<F: Fn(&mut Sprite) -> Option<SceneEvent>>(&mut self, effect: F) {
        let events = self
            .selected_sprites
            .iter()
            .filter_map(|id| effect(self.scene.sprite(*id)?))
            .collect::<Vec<SceneEvent>>();

        if !events.is_empty() {
            self.scene_event(SceneEvent::EventSet(events));
            self.changes.sprite_selected_change();
        }
    }

    fn grab_selection(&self, at: Point) -> HeldObject {
        if self.single_selected() {
            if let Some(s) = self.sprite_ref(self.selected_sprites[0]) {
                return HeldObject::grab_sprite(s, at);
            }
        }
        HeldObject::Selection(at)
    }

    /// Attempt to grab whatever lies at the cursor (`at`), if `add` is `true`
    /// adding to selection, else clearing selection and adding newly selected
    /// sprite. Returns a `HeldObject` which should be held after this click
    /// and an ID option which contains the newly selected sprite, if any.
    fn grab_at(&self, at: Point, add: bool) -> (HeldObject, Option<Id>) {
        if let Some(s) = self.scene.sprite_at_ref(at) {
            if self.has_selection() {
                if self.selected_sprites.contains(&s.id) {
                    return if self.single_selected() {
                        (HeldObject::grab_sprite(s, at), None)
                    } else {
                        (HeldObject::Selection(at), None)
                    };
                } else if add {
                    return (HeldObject::Selection(at), Some(s.id));
                }
            }
            (HeldObject::grab_sprite(s, at), Some(s.id))
        } else {
            (HeldObject::Marquee(at), None)
        }
    }

    pub fn grab(&mut self, at: Point, ctrl: bool) {
        let (held, new) = self.grab_at(at, ctrl);
        self.holding = held;

        if let Some(id) = new {
            if !self.is_selected(id) {
                if !ctrl {
                    self.clear_selection();
                }
                self.select(id);
            }
        }

        if self.holding.is_sprite() {
            self.start_move_group();
        }

        self.changes.sprite_change();
    }

    pub fn start_draw(&mut self, at: Point, ephemeral: bool) {
        self.clear_held_selection();
        if self.draw_details.shape.is_some() {
            self.new_held_shape(self.draw_details.shape.unwrap(), at);
        } else if let Some(id) = self.new_sprite_at(
            Some(SpriteVisual::Drawing(self.draw_details.drawing())),
            None,
            at,
        ) {
            self.start_move_group();
            self.holding = HeldObject::Drawing(id, ephemeral);
        }
    }

    fn update_held_sprite(&mut self, at: Point) {
        let holding = self.holding;
        let sprite = if let Some(s) = self.held_sprite() {
            s
        } else {
            return;
        };

        let event = match holding {
            HeldObject::Sprite(_, offset, _) => sprite.set_pos(at - offset),
            HeldObject::Anchor(_, dx, dy, _) => {
                let Point {
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
    }

    fn drag_selection(&mut self, to: Point) {
        let delta = if let HeldObject::Selection(from) = self.holding {
            to - from
        } else {
            return;
        };

        self.selection_effect(|s| Some(s.move_by(delta)));
        self.holding = HeldObject::Selection(to);
    }

    pub fn drag(&mut self, at: Point) {
        match self.holding {
            HeldObject::Drawing(id, _) => {
                if let Some(Some(event)) = self.scene.sprite(id).map(|s| s.add_drawing_point(at)) {
                    self.changes.sprite_change();
                    self.scene_event(event);
                }
            }
            HeldObject::Marquee(from) => {
                self.selection_marquee = Some(from.rect(at));
                self.changes.sprite_selected_change();
            }
            HeldObject::None => {}
            HeldObject::Selection(_) => self.drag_selection(at),
            HeldObject::Sprite(..) | HeldObject::Anchor(..) => self.update_held_sprite(at),
        };
    }

    pub fn sprite_ref(&self, id: Id) -> Option<&Sprite> {
        self.scene.sprite_ref(id)
    }

    pub fn sprite_at(&self, at: Point) -> Option<Id> {
        let id = self.scene.sprite_at_ref(at).map(|s| s.id)?;
        if self.is_selected(id) {
            Some(Self::SELECTION_ID)
        } else {
            Some(id)
        }
    }

    fn apply_ignore_threshold(&mut self, id: Id, starting_rect: Rect) -> bool {
        const IGNORE_THRESHOLD: f32 = 0.01;

        if let Some(sprite) = self.scene.sprite(id) {
            if sprite.rect.delta(starting_rect) < IGNORE_THRESHOLD {
                if sprite.rect != starting_rect {
                    let event = sprite.set_rect(starting_rect);
                    self.scene_event(event);
                }
                return true;
            }
        }
        false
    }

    fn finish_sprite_resize(&mut self, id: Id, starting_rect: Rect, snap_to_grid: bool) {
        if !self.apply_ignore_threshold(id, starting_rect) {
            if let Some(s) = self.scene.sprite(id) {
                if snap_to_grid {
                    let event = s.snap_size();
                    self.scene_event(event);
                } else {
                    let opt = s.enforce_min_size();
                    self.scene_option(opt);
                }
            }
        }
        self.changes.sprite_selected_change();
    }

    fn finish_sprite_drag(&mut self, id: Id, starting_rect: Rect, snap_to_grid: bool) {
        if !self.apply_ignore_threshold(id, starting_rect) && snap_to_grid {
            if let Some(s) = self.scene.sprite(id) {
                let event = s.snap_pos();
                self.scene_event(event);
            }
        }
        self.changes.sprite_selected_change();
    }

    fn finish_selection_drag(&mut self, snap_to_grid: bool) {
        if snap_to_grid {
            self.selection_effect(|s| Some(s.snap_pos()));
        }
        self.changes.sprite_selected_change();
    }

    fn finish_draw(&mut self, id: Id, ephemeral: bool) {
        if let Some(sprite) = self.scene.sprite(id) {
            // If this was just a single click, no line drawn, just remove it
            if sprite.n_drawing_points() == 1 || ephemeral {
                self.remove_sprite(id);
            } else {
                let opt = sprite.finish_drawing();
                self.scene_option(opt);
                self.end_move_group();
            }
        }
    }

    pub fn release(&mut self, alt: bool, ctrl: bool) {
        match self.holding {
            HeldObject::Drawing(id, ephemeral) => self.finish_draw(id, ephemeral),
            HeldObject::None => {}
            HeldObject::Marquee(_) => {
                if !ctrl {
                    self.clear_selection();
                }

                if let Some(region) = self.selection_marquee {
                    let mut selection = self.scene.sprites_in(region, alt);
                    self.select_multiple(&mut selection);
                }
                self.selection_marquee = None;
                self.changes.sprite_selected_change();
            }
            HeldObject::Selection(_) => self.finish_selection_drag(!alt),
            HeldObject::Sprite(id, _, start) => self.finish_sprite_drag(id, start, !alt),
            HeldObject::Anchor(id, _, _, start) => self.finish_sprite_resize(id, start, !alt),
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

        for id in &self.selected_sprites {
            if let Some(s) = self.scene.sprite(*id) {
                selections.push(s.rect);
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
            self.changes.all_change();
        }
    }

    fn replace_perms(&mut self, new: Perms) {
        self.perms = new;
    }

    pub fn replace_scene(&mut self, new: Scene) {
        self.scene = new;
        self.changes.all_change();
    }

    pub fn get_scene_details(&mut self) -> SceneDetails {
        SceneDetails::from(&self.scene)
    }

    pub fn scene_details(&mut self, details: SceneDetails) {
        details.update_scene(&mut self.scene);
        self.changes.sprite_change();
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
    }

    pub fn remove_layer(&mut self, layer: Id) {
        let opt = self.scene.remove_layer(layer);
        self.scene_option(opt);
        self.changes.all_change();
    }

    pub fn rename_layer(&mut self, layer: Id, title: String) {
        let opt = self.scene.rename_layer(layer, title);
        self.scene_option(opt);
    }

    pub fn select_layer(&mut self, layer: Id) {
        self.selected_layer = layer;
    }

    pub fn set_layer_visible(&mut self, layer: Id, visible: bool) {
        if let Some(l) = self.scene.layer(layer) {
            let opt = l.set_visible(visible);
            let changed = !l.sprites.is_empty();
            self.changes.sprite_change_if(changed);
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
        self.changes.all_change();
    }

    fn new_sprite_common(
        &mut self,
        visual: Option<SpriteVisual>,
        layer: Option<Id>,
        at: Option<Point>,
    ) -> Option<Id> {
        let layer = layer.unwrap_or(self.selected_layer);

        let opt = if let Some(at) = at {
            self.scene.new_sprite_at(visual, layer, at)
        } else {
            self.scene.new_sprite(visual, layer)
        };

        let ret = if let Some(SceneEvent::SpriteNew(s, _)) = &opt {
            Some(s.id)
        } else {
            None
        };

        self.scene_option(opt);
        ret
    }

    fn new_sprite(&mut self, visual: Option<SpriteVisual>, layer: Option<Id>) -> Option<Id> {
        self.new_sprite_common(visual, layer, None)
    }

    pub fn new_sprite_at(
        &mut self,
        visual: Option<SpriteVisual>,
        layer: Option<Id>,
        at: Point,
    ) -> Option<Id> {
        self.new_sprite_common(visual, layer, Some(at))
    }

    pub fn new_held_shape(&mut self, shape: SpriteShape, at: Point) {
        self.clear_held_selection();
        if let Some(id) = self.new_sprite(
            Some(SpriteVisual::Solid {
                colour: self.draw_details.colour(),
                shape,
                stroke: Sprite::SOLID_STROKE,
            }),
            Some(self.selected_layer),
        ) {
            let opt = self.scene.sprite(id).map(|s| {
                self.holding = HeldObject::Anchor(s.id, 1, 1, s.rect);
                s.set_rect(Rect::new(at.x, at.y, 0.0, 0.0))
            });
            self.scene_option(opt);
        }
    }

    pub fn clone_sprite(&mut self, sprite: Id) {
        if sprite == Self::SELECTION_ID {
            let mut events = vec![];
            for id in &self.selected_sprites {
                if let Some(event) = self.scene.clone_sprite(*id) {
                    events.push(event);
                }
            }

            if !events.is_empty() {
                self.scene_event(SceneEvent::EventSet(events));
            }
        } else {
            let opt = self.scene.clone_sprite(sprite);
            self.scene_option(opt);
        }
    }

    pub fn remove_sprite(&mut self, sprite: Id) {
        if sprite == Self::SELECTION_ID {
            let event = self.scene.remove_sprites(&self.selected_sprites);
            self.scene_event(event);
            self.clear_selection();
        } else {
            let opt = self.scene.remove_sprite(sprite);
            self.scene_option(opt);
        }
    }

    pub fn remove_selection(&mut self) {
        self.remove_sprite(Self::SELECTION_ID);
    }

    pub fn sprite_layer(&mut self, sprite: Id, layer: Id) {
        if sprite == Self::SELECTION_ID {
            let event = self.scene.sprites_layer(&self.selected_sprites, layer);
            self.scene_event(event);
        } else {
            let opt = self.scene.sprite_layer(sprite, layer);
            self.scene_option(opt);
        }
    }

    pub fn sprite_dimension(&mut self, sprite: Id, dimension: Dimension, value: f32) {
        if sprite == Self::SELECTION_ID {
            self.selection_effect(|s| Some(s.set_dimension(dimension, value)));
        } else if let Some(s) = self.scene.sprite(sprite) {
            let event = s.set_dimension(dimension, value);
            self.scene_event(event);
            self.changes.sprite_selected_change();
        }
    }

    pub fn sprite_rect(&mut self, sprite: Id, rect: Rect) {
        let opt = self.scene.sprite(sprite).map(|s| s.set_rect(rect));
        self.scene_option(opt);
    }

    fn selected_id(&self) -> Option<Id> {
        match self.selected_sprites.len() {
            1 => Some(self.selected_sprites[0]),
            2.. => Some(Self::SELECTION_ID),
            _ => None,
        }
    }

    pub fn selected_details(&self) -> Option<SpriteDetails> {
        let id = self.selected_id()?;
        if id == Self::SELECTION_ID {
            if self.has_selection() {
                let sprite = self.sprite_ref(self.selected_sprites[0])?;
                let mut details = SpriteDetails::from(id, sprite);

                for id in &self.selected_sprites[1..] {
                    if let Some(sprite) = self.sprite_ref(*id) {
                        details.common(sprite);
                    }
                }

                Some(details)
            } else {
                None
            }
        } else {
            Some(SpriteDetails::from(id, self.sprite_ref(id)?))
        }
    }

    pub fn sprite_details(&mut self, id: Id, details: SpriteDetails) {
        if id == Self::SELECTION_ID {
            self.selection_effect(|s| details.update_sprite(s));
        } else if let Some(sprite) = self.scene.sprite(id) {
            let opt = details.update_sprite(sprite);
            self.scene_option(opt);
        }
    }

    pub fn move_selection(&mut self, delta: Point) {
        if delta.x == 0.0 && delta.y == 0.0 {
            return;
        }
        self.selection_effect(|s| Some(s.move_by(delta)));
    }

    pub fn update_draw_details(&mut self, details: SpriteDetails) {
        self.draw_details.update_from(&details);
    }
}
