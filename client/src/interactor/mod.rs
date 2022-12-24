use bincode::serialize;
use scene::comms::ServerEvent;

use crate::scene::{
    comms::SceneEvent, perms::Perms, Dimension, Id, Layer, Point, Rect, Scene, Sprite, SpriteShape,
    SpriteVisual,
};
use crate::{bridge::Cursor, client::Client};

pub mod changes;
pub mod details;
pub mod history;
pub mod holding;

pub struct Interactor {
    pub changes: changes::Changes,
    pub role: scene::perms::Role,
    copied: Option<Vec<Sprite>>,
    draw_details: details::SpriteDetails,
    fog_brush: u32,
    history: history::History,
    holding: holding::HeldObject,
    perms: Perms,
    scene: Scene,
    selected_layer: Id,
    selected_sprites: Vec<Id>,

    /// Whether all sprites in the selection are aligned to the grid.
    selection_aligned: bool,
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
            changes: changes::Changes::new(),
            role: scene::perms::Role::Owner,
            copied: None,
            draw_details: details::SpriteDetails::default(),
            fog_brush: 1,
            history: history::History::new(client),
            holding: holding::HeldObject::None,
            perms: Perms::new(),
            scene,
            selected_layer,
            selected_sprites: Vec::new(),
            selection_aligned: true,
            selection_marquee: None,
            user: scene::perms::CANONICAL_UPDATER,
        }
    }

    pub fn process_server_events(&mut self) {
        if let Some(events) = self.history.server_events() {
            for event in events {
                self.process_server_event(event);
                self.changes.sprite_change();
            }
        }
    }

    fn process_server_event(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::Approval(id) => self.history.approve_event(id),
            ServerEvent::Rejection(id) => {
                if let Some(event) = self.history.take_event(id) {
                    self.unwind_event(event)
                }
            }
            ServerEvent::PermsChange(perms) => self.replace_perms(perms),
            ServerEvent::PermsUpdate(perms_event) => {
                let is_role = matches!(perms_event, scene::comms::PermsEvent::RoleChange(..));
                self.perms
                    .handle_event(scene::perms::CANONICAL_UPDATER, perms_event);

                if is_role {
                    self.update_role();
                }
            }
            ServerEvent::SceneChange(scene) => self.replace_scene(scene),
            ServerEvent::SceneList(scenes, current) => {
                crate::bridge::populate_change_scene(&scenes, &current).ok();
            }
            ServerEvent::SceneUpdate(scene_event) => {
                self.changes.layer_change_if(scene_event.is_layer());
                self.scene.apply_event(scene_event);
            }
            ServerEvent::UserId(id) => {
                self.user = id;
                self.update_role();
            }
        }
    }

    fn unwind_event(&mut self, event: SceneEvent) {
        // If we got rejected while dragging a sprite, release that
        // sprite to prevent visual jittering and allow the position to
        // reset.
        if self.held_id() == event.item() {
            self.holding = holding::HeldObject::None;
        }

        self.changes.layer_change_if(event.is_layer());
        self.changes.sprite_selected_change();
        self.scene.unwind_event(event);
    }

    fn scene_event(&mut self, event: SceneEvent) {
        if self
            .perms
            .permitted(self.user, &event, self.scene.event_layer(&event))
        {
            self.history.issue_event(event.clone());

            self.changes.layer_change_if(event.is_layer());
            self.changes.sprite_change_if(event.is_sprite());
            self.changes.sprite_change_if(event.is_fog());
            if let Some(id) = event.item() {
                self.changes.selected_change_if(self.is_selected(id));
            }
        } else {
            crate::bridge::flog!("forbidden: {event:?}");
            self.scene.unwind_event(event);
        }
    }

    fn scene_option(&mut self, event_option: Option<SceneEvent>) {
        if let Some(event) = event_option {
            self.scene_event(event);
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
                self.history.issue_event_no_history(event.clone());
                self.changes.layer_change_if(layers_changed);
                self.changes.sprite_selected_change();
            }
            self.history.issue_redo(opt);
        }
    }

    pub fn redo(&mut self) {
        if let Some(event) = self.history.pop_redo() {
            if let Some(event) = self.scene.unwind_event(event) {
                let layers_changed = event.is_layer();
                self.history.issue_event(event);
                self.changes.layer_change_if(layers_changed);
                self.changes.sprite_selected_change();
            }
        }
    }

    pub fn copy(&mut self) {
        if !self.has_selection() {
            return;
        }

        let mut copied = Vec::with_capacity(self.selected_sprites.len());
        let mut xmin = std::f32::MAX;
        let mut ymin = std::f32::MAX;
        for id in &self.selected_sprites {
            if let Some(sprite) = self.scene.sprite_ref(*id) {
                copied.push(sprite.clone());
                xmin = xmin.min(sprite.rect.x);
                ymin = ymin.min(sprite.rect.y);
            }
        }

        for sprite in &mut copied {
            sprite.rect.x -= xmin;
            sprite.rect.y -= ymin;
        }

        self.copied = Some(copied);
    }

    pub fn paste(&mut self, at: Point) {
        if self.copied.is_none() {
            return;
        } else {
            self.clear_selection();
        };

        let mut events = Vec::with_capacity(self.copied.as_ref().unwrap().len());

        // Place new sprite at cursor.
        let delta = at.round();
        for s in self.copied.as_ref().unwrap().clone() {
            let at = s.rect.translate(delta);
            if let Some(event) =
                self.scene
                    .new_sprite_at(Some(s.visual.clone()), self.selected_layer, at)
            {
                if let SceneEvent::SpriteNew(s, _) = &event {
                    self.select(s.id);
                }
                events.push(event);
            }
        }

        self.scene_event(SceneEvent::EventSet(events));
    }

    fn update_role(&mut self) {
        self.role = self.perms.get_role(self.user);
        crate::bridge::set_role(self.role);
    }

    pub fn cursor(&self) -> Cursor {
        self.holding.cursor()
    }

    pub fn cursor_at(&self, at: Point, ctrl: bool) -> Cursor {
        if matches!(self.holding, holding::HeldObject::None) {
            match self.grab_at(at, ctrl).0 {
                holding::HeldObject::Sprite(..) => Cursor::Pointer,
                h => h.cursor(),
            }
        } else {
            self.cursor()
        }
    }

    fn held_id(&self) -> Option<Id> {
        match self.holding {
            holding::HeldObject::Sprite(id, ..) => Some(id),
            holding::HeldObject::Anchor(id, ..) => Some(id),
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
        self.selection_aligned = true;
    }

    fn clear_held_selection(&mut self) {
        self.holding = holding::HeldObject::None;
        self.clear_selection();
    }

    fn select(&mut self, id: Id) {
        if !self.is_selected(id) && self.perms.selectable(self.user, id) {
            self.selected_sprites.push(id);
            if let Some(s) = self.sprite_ref(id) {
                self.selection_aligned = self.selection_aligned && s.rect.is_aligned();
            }
            self.changes.sprite_selected_change();
        }
    }

    fn select_multiple(&mut self, ids: &[Id]) {
        for id in ids {
            self.select(*id);
        }
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

    fn grab_selection(&self, at: Point) -> holding::HeldObject {
        if self.single_selected() {
            if let Some(s) = self.sprite_ref(self.selected_sprites[0]) {
                return holding::HeldObject::grab_sprite(s, at);
            }
        }
        holding::HeldObject::Selection(at)
    }

    /// Attempt to grab whatever lies at the cursor (`at`), if `add` is `true`
    /// adding to selection, else clearing selection and adding newly selected
    /// sprite. Returns a `holding::HeldObject` which should be held after this click
    /// and an ID option which contains the newly selected sprite, if any.
    fn grab_at(&self, at: Point, add: bool) -> (holding::HeldObject, Option<Id>) {
        if let Some(s) = self.scene.sprite_at_ref(at) {
            if self.has_selection() {
                if self.is_selected(s.id) {
                    return if self.single_selected() {
                        (holding::HeldObject::grab_sprite(s, at), None)
                    } else {
                        (holding::HeldObject::Selection(at), None)
                    };
                } else if add {
                    return (holding::HeldObject::Selection(at), Some(s.id));
                }
            }
            (holding::HeldObject::grab_sprite(s, at), Some(s.id))
        } else {
            (holding::HeldObject::Marquee(at), None)
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
            self.history.start_move_group();
        }

        self.changes.sprite_change();
    }

    pub fn start_draw(&mut self, at: Point, ephemeral: bool, alt: bool) {
        self.clear_held_selection();
        if self.draw_details.shape.is_some() {
            self.new_held_shape(self.draw_details.shape.unwrap(), at, !alt);
        } else if let Some(id) = self.new_sprite_at(
            Some(self.draw_details.drawing()),
            None,
            Rect::at(at, Sprite::DEFAULT_WIDTH, Sprite::DEFAULT_HEIGHT),
        ) {
            self.history.start_move_group();
            self.holding = holding::HeldObject::Drawing(id, ephemeral);
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
            holding::HeldObject::Sprite(_, offset, _) => sprite.set_pos(at - offset),
            holding::HeldObject::Anchor(_, dx, dy, _) => {
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
        let delta = if let holding::HeldObject::Selection(from) = self.holding {
            to - from
        } else {
            return;
        };

        self.selection_effect(|s| Some(s.move_by(delta)));
        self.holding = holding::HeldObject::Selection(to);
    }

    pub fn drag(&mut self, at: Point) {
        match self.holding {
            holding::HeldObject::Drawing(id, _) => {
                if let Some(Some(event)) = self.scene.sprite(id).map(|s| s.add_drawing_point(at)) {
                    self.changes.sprite_change();
                    self.scene_event(event);
                }
            }
            holding::HeldObject::Marquee(from) => {
                self.selection_marquee = Some(from.rect(at));
                self.changes.sprite_selected_change();
            }
            holding::HeldObject::None => {}
            holding::HeldObject::Selection(_) => self.drag_selection(at),
            holding::HeldObject::Sprite(..) | holding::HeldObject::Anchor(..) => {
                self.update_held_sprite(at)
            }
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

    fn finish_sprite_resize(&mut self, id: Id, starting_rect: Rect, switch_align: bool) {
        if !self.apply_ignore_threshold(id, starting_rect) {
            if let Some(s) = self.scene.sprite(id) {
                if starting_rect.is_aligned() ^ switch_align {
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

    fn finish_sprite_drag(&mut self, id: Id, starting_rect: Rect, switch_align: bool) {
        if !self.apply_ignore_threshold(id, starting_rect)
            && (starting_rect.is_aligned() ^ switch_align)
        {
            if let Some(s) = self.scene.sprite(id) {
                let event = s.snap_pos();
                self.scene_event(event);
            }
        }
        self.changes.sprite_selected_change();
    }

    fn finish_selection_drag(&mut self, switch_align: bool) {
        if self.selection_aligned ^ switch_align {
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
                self.history.end_move_group();
            }
        }
    }

    pub fn release(&mut self, alt: bool, ctrl: bool) {
        match self.holding {
            holding::HeldObject::Drawing(id, ephemeral) => self.finish_draw(id, ephemeral),
            holding::HeldObject::None => {}
            holding::HeldObject::Marquee(_) => {
                if !ctrl {
                    self.clear_selection();
                }

                if let Some(region) = self.selection_marquee {
                    let selection = self.scene.sprites_in(region, alt);
                    self.select_multiple(&selection);
                }
                self.selection_marquee = None;
                self.changes.sprite_selected_change();
            }
            holding::HeldObject::Selection(_) => self.finish_selection_drag(alt),
            holding::HeldObject::Sprite(id, _, start) => self.finish_sprite_drag(id, start, alt),
            holding::HeldObject::Anchor(id, _, _, start) => {
                self.finish_sprite_resize(id, start, alt)
            }
        };

        if self.holding.is_sprite() {
            self.history.end_move_group();
        }

        self.holding = holding::HeldObject::None;
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
            w: self.scene.w() as f32,
            h: self.scene.h() as f32,
        }
    }

    #[must_use]
    pub fn fog(&self) -> &scene::Fog {
        &self.scene.fog
    }

    pub fn set_fog_brush(&mut self, size: u32) {
        self.fog_brush = size;
    }

    pub fn set_fog(&mut self, at: Point, ctrl: bool) {
        let x = at.x as u32;
        let y = at.y as u32;
        let event = self.scene.fog.set_square(x, y, self.fog_brush, ctrl);
        self.scene_event(event);
    }

    #[must_use]
    pub fn export(&self) -> Vec<u8> {
        match serialize(&self.scene) {
            Ok(v) => v,
            Err(_) => vec![],
        }
    }

    pub fn new_scene(&mut self, id: Id) {
        self.scene = Scene::new();
        if id != 0 {
            self.scene.project = Some(id);
        }
        self.changes.all_change();
    }

    pub fn change_scene(&mut self, scene_key: String) {
        self.history.change_scene(scene_key);
    }

    fn replace_perms(&mut self, new: Perms) {
        self.perms = new;
        self.update_role();
    }

    pub fn replace_scene(&mut self, new: Scene) {
        self.scene = new;
        self.changes.all_change();
        crate::bridge::set_scene_details(self.get_scene_details());
    }

    pub fn get_scene_details(&mut self) -> details::SceneDetails {
        details::SceneDetails::from(&self.scene)
    }

    pub fn scene_details(&mut self, details: details::SceneDetails) {
        let opt = details.update_scene(&mut self.scene);
        self.scene_option(opt);
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
        at: Option<Rect>,
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
        at: Rect,
    ) -> Option<Id> {
        self.new_sprite_common(visual, layer, Some(at))
    }

    pub fn new_held_shape(&mut self, shape: SpriteShape, at: Point, snap_to_grid: bool) {
        self.clear_held_selection();
        if let Some(id) = self.new_sprite(
            Some(SpriteVisual::Solid {
                colour: self.draw_details.colour(),
                shape,
                stroke: Sprite::SOLID_STROKE,
            }),
            Some(self.selected_layer),
        ) {
            let at = if snap_to_grid { at.round() } else { at };
            let opt = self.scene.sprite(id).map(|s| {
                self.holding = holding::HeldObject::Anchor(s.id, 1, 1, s.rect);
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
            if self.single_selected() {
                self.remove_sprite(self.selected_sprites[0]);
            } else {
                let event = self.scene.remove_sprites(&self.selected_sprites);
                self.scene_event(event);
            }
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

    pub fn selected_details(&self) -> Option<details::SpriteDetails> {
        let id = self.selected_id()?;
        if id == Self::SELECTION_ID {
            if self.has_selection() {
                let sprite = self.sprite_ref(self.selected_sprites[0])?;
                let mut details = details::SpriteDetails::from(id, sprite);

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
            Some(details::SpriteDetails::from(id, self.sprite_ref(id)?))
        }
    }

    pub fn sprite_details(&mut self, id: Id, details: details::SpriteDetails) {
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

    pub fn update_draw_details(&mut self, details: details::SpriteDetails) {
        self.draw_details.update_from(&details);
    }
}
