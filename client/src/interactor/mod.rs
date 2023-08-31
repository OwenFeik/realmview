use bincode::serialize;
use scene::comms::ServerEvent;
use scene::Outline;

use self::holding::HeldObject;
use crate::dom::menu::CanvasDropdownEvent;
use crate::dom::menu::LayerInfo;
use crate::scene::{
    comms::SceneEvent, perms::Perms, Dimension, Id, Layer, Point, Rect, Scene, Shape, Sprite,
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
    fog_brush: f32,
    history: history::History,
    holding: HeldObject,
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
    pub const DEFAULT_FOG_BRUSH: f32 = 1.0;

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
            fog_brush: Self::DEFAULT_FOG_BRUSH,
            history: history::History::new(client),
            holding: HeldObject::None,
            perms: Perms::new(),
            scene,
            selected_layer,
            selected_sprites: Vec::new(),
            selection_aligned: true,
            selection_marquee: None,
            user: scene::perms::CANONICAL_UPDATER,
        }
    }

    pub fn scene_key(&self) -> Option<String> {
        self.scene.key.clone()
    }

    pub fn process_server_events(&mut self) -> Option<(Vec<(String, String)>, String)> {
        let mut ret = None;
        if let Some(events) = self.history.server_events() {
            for event in events {
                let result = self.process_server_event(event);
                if result.is_some() {
                    ret = result;
                }
                self.changes.sprite_change();
            }
        }
        ret
    }

    fn process_server_event(
        &mut self,
        event: ServerEvent,
    ) -> Option<(Vec<(String, String)>, String)> {
        match event {
            ServerEvent::Approval(id) => self.history.approve_event(id),
            ServerEvent::EventSet(events) => {
                for event in events {
                    self.process_server_event(event);
                }
            }
            ServerEvent::GameOver => {
                crate::bridge::game_over_redirect();
            }
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
            ServerEvent::SceneChange(scene) => {
                self.replace_scene(scene);
            }
            ServerEvent::SceneList(scenes, current) => {
                return Some((scenes, current));
            }
            ServerEvent::SceneUpdate(scene_event) => {
                self.changes.layer_change_if(scene_event.is_layer());
                self.scene.apply_event(scene_event);
            }
            ServerEvent::SelectedLayer(layer) => {
                self.selected_layer = layer;
            }
            ServerEvent::UserId(id) => {
                self.user = id;
                self.update_role();
            }
        };

        None
    }

    fn unwind_event(&mut self, event: SceneEvent) {
        // If we got rejected while dragging a sprite, release that
        // sprite to prevent visual jittering and allow the position to
        // reset.
        if self.held_id() == event.item() {
            self.holding = HeldObject::None;
        }

        self.changes.layer_change_if(event.is_layer());
        self.changes.sprite_selected_change();
        self.scene.unwind_event(event);
    }

    fn change_if(&mut self, event: &SceneEvent) {
        self.changes.layer_change_if(event.is_layer());
        self.changes.sprite_change_if(event.is_sprite());
        self.changes.sprite_change_if(event.is_fog());
        if let Some(id) = event.item() {
            self.changes.selected_change_if(self.is_selected(id));
        }

        if let SceneEvent::EventSet(events) = event {
            events.iter().for_each(|e| self.change_if(e));
        }
    }

    fn scene_event(&mut self, event: SceneEvent) {
        if self
            .perms
            .permitted(self.user, &event, self.scene.event_layer(&event))
        {
            self.change_if(&event);
            self.history.issue_event(event);
        } else {
            crate::bridge::flog!("forbidden: {event:?}");
            self.scene.unwind_event(event);
        }
    }

    fn scene_events(&mut self, events: Vec<SceneEvent>) {
        if events.is_empty() {
            return;
        }

        if events.len() == 1 {
            self.scene_option(events.into_iter().next());
        } else {
            self.scene_event(SceneEvent::EventSet(events));
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

    pub fn save_required(&self) -> bool {
        self.history.save_required()
    }

    pub fn save_done(&mut self) {
        self.history.clear_modified();
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

        self.scene_events(events);
    }

    fn update_role(&mut self) {
        self.role = self.perms.get_role(self.user);
        self.changes.role_change();
    }

    pub fn cursor(&self, at: Point) -> Cursor {
        self.holding.cursor(at)
    }

    pub fn cursor_at(&self, at: Point, ctrl: bool) -> Cursor {
        if matches!(self.holding, HeldObject::None) {
            match self.grab_at(at, ctrl).0 {
                HeldObject::Sprite(..) => Cursor::Pointer,
                h => h.cursor(at),
            }
        } else {
            self.cursor(at)
        }
    }

    fn held_id(&self) -> Option<Id> {
        self.holding.held_id()
    }

    fn held_sprite(&self) -> Option<&Sprite> {
        match self.held_id() {
            Some(id) => self.scene.sprite_ref(id),
            None => None,
        }
    }

    fn held_sprite_mut(&mut self) -> Option<&mut Sprite> {
        self.held_id().and_then(|id| self.scene.sprite(id))
    }

    pub fn has_selection(&self) -> bool {
        !self.selected_sprites.is_empty()
    }

    fn is_selected(&self, id: Id) -> bool {
        id == Self::SELECTION_ID || self.selected_sprites.contains(&id)
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
        self.holding = HeldObject::None;
        self.clear_selection();
    }

    /// Common handling for sprites in groups and single sprites. Only called
    /// from select.
    fn _select(&mut self, id: Id, require_visible: bool) {
        if !self.is_selected(id) && self.perms.selectable(self.user, id) {
            if let Some(s) = self.sprite_ref(id) {
                if !require_visible || self.role.editor() || !self.scene.fog.rect_occluded(s.rect) {
                    self.selection_aligned = self.selection_aligned && s.rect.is_aligned();
                    self.selected_sprites.push(id);
                    self.changes.sprite_selected_change();
                }
            }
        }
    }

    fn select(&mut self, id: Id) {
        if let Some(g) = self.scene.sprite_group(id).map(|g| g.sprites().to_owned()) {
            g.iter().for_each(|id| self._select(*id, false));
        } else {
            self._select(id, true);
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

        self.scene_events(events);
    }

    fn grab_selection(&self, at: Point) -> HeldObject {
        if self.single_selected() {
            if let Some(s) = self.sprite_ref(self.selected_sprites[0]) {
                return HeldObject::grab_sprite(s, at);
            }
        }
        HeldObject::Selection(at)
    }

    fn sprite_to_grab_at(&self, at: Point) -> Option<&Sprite> {
        if self.single_selected() {
            self.scene.sprite_near(at, HeldObject::ANCHOR_RADIUS)
        } else {
            self.scene.sprite_at_ref(at)
        }
    }

    /// Attempt to grab whatever lies at the cursor (`at`), if `add` is `true`
    /// adding to selection, else clearing selection and adding newly selected
    /// sprite. Returns a `HeldObject` which should be held after this click
    /// and an ID option which contains the newly selected sprite, if any.
    fn grab_at(&self, at: Point, add: bool) -> (HeldObject, Option<Id>) {
        if let Some(s) = self.sprite_to_grab_at(at) {
            if !self.role.editor() && self.scene.fog.rect_occluded(s.rect) && self.scene.fog.active
            {
                (HeldObject::Marquee(at), None)
            } else {
                if self.has_selection() {
                    if self.is_selected(s.id) {
                        return if self.single_selected() {
                            if self.scene.sprite_group(s.id).is_some() {
                                (HeldObject::Selection(at), None)
                            } else {
                                (HeldObject::grab_sprite(s, at), None)
                            }
                        } else {
                            (HeldObject::Selection(at), None)
                        };
                    } else if add {
                        return (HeldObject::Selection(at), Some(s.id));
                    }
                }

                if self.scene.sprite_group(s.id).is_some() {
                    (HeldObject::Selection(at), Some(s.id))
                } else {
                    (HeldObject::sprite(s, at), Some(s.id))
                }
            }
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
            self.history.start_move_group();
        }

        self.changes.sprite_change();
    }

    pub fn start_draw(
        &mut self,
        at: Point,
        ephemeral: bool,
        alt: bool,
        details: details::SpriteDetails,
        tool: crate::viewport::DrawTool,
    ) {
        use crate::viewport::DrawTool;

        self.clear_held_selection();

        match tool {
            DrawTool::Circle => {
                self.new_held_shape(Shape::Ellipse, at, !alt, ephemeral, details);
                if let HeldObject::Anchor(id, .., ephemeral) = self.holding {
                    self.holding = HeldObject::Circle(id, at, ephemeral);
                }
            }
            DrawTool::Ellipse | DrawTool::Rectangle => {
                if details.shape.is_some() {
                    self.new_held_shape(details.shape.unwrap(), at, !alt, ephemeral, details);
                }
            }
            DrawTool::Freehand | DrawTool::Line => {
                let mut visual = details.drawing();
                let drawing_id = self.scene.start_drawing();
                if let SpriteVisual::Drawing { drawing, .. } = &mut visual {
                    *drawing = drawing_id;
                }

                if let Some(sprite_id) = self.new_sprite_at(
                    Some(visual),
                    None,
                    Rect::at(Point::ORIGIN, Sprite::DEFAULT_WIDTH, Sprite::DEFAULT_HEIGHT),
                ) {
                    self.history.start_move_group();
                    self.holding = HeldObject::Drawing(drawing_id, sprite_id, ephemeral, !alt);
                }
            }
        }
    }

    fn update_held_sprite(&mut self, at: Point, maintain_aspect_ratio: bool) {
        let held = self.holding.clone();
        let sprite = if let Some(s) = self.held_sprite_mut() {
            s
        } else {
            return;
        };

        let event = match held {
            HeldObject::Circle(_, centre, _) => {
                let r = at.dist(centre);
                sprite.set_rect(Rect {
                    x: centre.x - r,
                    y: centre.y - r,
                    w: 2.0 * r,
                    h: 2.0 * r,
                })
            }
            HeldObject::Sprite(_, offset, _) => sprite.set_pos(at - offset),
            HeldObject::Anchor(_, dx, dy, starting_rect, _) => {
                let Point {
                    x: delta_x,
                    y: delta_y,
                } = at - sprite.anchor_point(dx, dy);
                let x = sprite.rect.x + (if dx == -1 { delta_x } else { 0.0 });
                let y = sprite.rect.y + (if dy == -1 { delta_y } else { 0.0 });
                let w = delta_x * (dx as f32) + sprite.rect.w;
                let h = delta_y * (dy as f32) + sprite.rect.h;
                let rect = Rect { x, y, w, h };
                
                if maintain_aspect_ratio {
                    sprite.set_rect(rect.match_aspect(starting_rect))
                } else {
                    sprite.set_rect(rect)
                }
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

    pub fn drag(&mut self, at: Point, shift: bool) {
        match self.holding {
            HeldObject::Drawing(d, _sprite, _ephemeral, _measurement) => {
                let opt = self.scene.add_drawing_point(d, at);
                self.scene_option(opt);
            }
            HeldObject::Marquee(from) => {
                self.selection_marquee = Some(from.rect(at));
                self.changes.sprite_selected_change();
            }
            HeldObject::None => {}
            HeldObject::Selection(_) => self.drag_selection(at),
            HeldObject::Anchor(..) | HeldObject::Circle(..) | HeldObject::Sprite(..) => {
                self.update_held_sprite(at, shift)
            }
        };
    }

    fn add_sprite_measurements(&self, sprite: Id, to: &mut Vec<(Point, f32)>) {
        if let Some(sprite) = self.sprite_ref(sprite) {
            match sprite.visual {
                SpriteVisual::Drawing { drawing, mode, .. } => {
                    if let Some(drawing) = self.scene.get_drawing(drawing) {
                        to.push((
                            drawing.points.rect().top_left() + sprite.rect.top_left()
                                - Point::same(0.5),
                            drawing.length(mode),
                        ));
                    }
                }
                SpriteVisual::Shape { shape, .. } | SpriteVisual::Texture { shape, .. } => {
                    match shape {
                        Shape::Ellipse | Shape::Hexagon if sprite.rect.w == sprite.rect.h => {
                            if sprite.rect.w == sprite.rect.h {
                                to.push((sprite.rect.centre(), sprite.rect.w / 2.0));
                            }
                        }
                        _ => {
                            let rect = sprite.rect.positive_dimensions();
                            let centre = rect.centre();
                            let above = Point::new(centre.x, rect.y - 0.5);
                            let left = Point::new(rect.x - 0.5, centre.y);
                            to.push((above, sprite.rect.w.abs()));
                            to.push((left, sprite.rect.h.abs()));
                        }
                    }
                }
            }
        }
    }

    pub fn active_measurements(&self) -> Vec<(Point, f32)> {
        let mut measurements = Vec::new();

        if let Some(sprite) = self.held_id() {
            self.add_sprite_measurements(sprite, &mut measurements);
        }

        if self.single_selected() && let Some(sprite) = self.selected_id() {
            self.add_sprite_measurements(sprite, &mut measurements);
        }

        measurements
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

    pub fn select_at(&mut self, at: Point, add: bool) -> bool {
        if let Some(id) = self.sprite_at(at) {
            if !add && !self.is_selected(id) && self.has_selection() {
                self.clear_selection();
            }
            self.select(id);
            true
        } else {
            false
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

    fn finish_draw(&mut self, drawing: Id, sprite: Id) {
        let opt = self.scene.finish_drawing(drawing, sprite);
        self.scene_option(opt);
        self.history.end_move_group();
    }

    fn finish_circle(&mut self, id: Id, snap_to_grid: bool) {
        if snap_to_grid && let Some(sprite) = self.scene.sprite(id) {
            let event = sprite.snap_size();
            self.scene_event(event);
        }
    }

    pub fn release(&mut self, alt: bool, ctrl: bool) {
        match self.holding {
            HeldObject::Anchor(sprite, _, _, _, true)
            | HeldObject::Circle(sprite, _, true)
            | HeldObject::Drawing(_, sprite, true, _) => {
                // Ephemeral
                self.remove_sprite(sprite);
                self.history.erase_item(sprite);
            }
            HeldObject::Circle(id, _, _) => self.finish_circle(id, !alt),
            HeldObject::Drawing(drawing, sprite, _, _) => self.finish_draw(drawing, sprite),
            HeldObject::None => {}
            HeldObject::Marquee(_) => {
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
            HeldObject::Selection(_) => self.finish_selection_drag(alt),
            HeldObject::Sprite(id, _, start) => self.finish_sprite_drag(id, start, alt),
            HeldObject::Anchor(id, _, _, start, _) => self.finish_sprite_resize(id, start, alt),
        };

        if self.holding.is_sprite() {
            self.history.end_move_group();
        }

        self.holding = HeldObject::None;
    }

    #[must_use]
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    #[must_use]
    pub fn layers(&self) -> &[Layer] {
        &self.scene.layers
    }

    #[must_use]
    pub fn selected_layer(&self) -> Id {
        self.selected_layer
    }

    #[must_use]
    pub fn selections(&mut self) -> Vec<Outline> {
        let mut selections = vec![];

        // Show outlines around all sprites.
        for id in &self.selected_sprites {
            if let Some(sprite) = self.scene.sprite(*id) {
                selections.push(sprite.outline());
            }
        }

        // Show selection anchors if a single sprite is selected.
        if self.single_selected() {
            if let Some(sprite) = self
                .selected_sprites
                .first()
                .and_then(|&id| self.scene.sprite(id))
            {
                for point in HeldObject::anchors(sprite) {
                    selections.push(Outline {
                        rect: Rect::around(point, HeldObject::ANCHOR_RADIUS),
                        shape: Shape::Ellipse,
                    })
                }
            }
        }

        // Don't want a 1*1 square showing while the user draws lines.
        if !matches!(self.holding, HeldObject::Drawing(..)) {
            if let Some(sprite) = self.held_sprite() {
                selections.push(sprite.outline());
            }
        }

        if let Some(rect) = self.selection_marquee {
            selections.push(Outline::rect(rect));
        }
        selections
    }

    #[must_use]
    pub fn dimensions(&self) -> (u32, u32) {
        (self.scene.w(), self.scene.h())
    }

    #[must_use]
    pub fn fog(&self) -> &scene::Fog {
        &self.scene.fog
    }

    pub fn get_fog_brush(&self) -> f32 {
        self.fog_brush
    }

    pub fn set_fog_brush(&mut self, size: f32) {
        self.fog_brush = size;
    }

    pub fn set_fog(&mut self, at: Point, ctrl: bool) {
        let event = self.scene.fog.set_circle(at, self.fog_brush, ctrl);
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

    /// Returns true if the scene change can be effected by the client, else
    /// false.
    pub fn change_scene(&mut self, scene_key: String) -> bool {
        self.history.change_scene(scene_key)
    }

    fn replace_perms(&mut self, new: Perms) {
        self.perms = new;
        self.update_role();
    }

    pub fn replace_scene(&mut self, new: Scene) {
        self.selected_sprites.clear();
        self.scene = new;
        self.changes.all_change();
    }

    pub fn get_scene_details(&self) -> details::SceneDetails {
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

        if layer == self.selected_layer {
            self.selected_layer = self.scene.first_layer();
        }
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

    pub fn layer_info(&self) -> Vec<LayerInfo> {
        self.scene.layers.iter().map(LayerInfo::from).collect()
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

    pub fn new_held_shape(
        &mut self,
        shape: Shape,
        at: Point,
        snap_to_grid: bool,
        ephemeral: bool,
        details: details::SpriteDetails,
    ) {
        self.clear_held_selection();
        let at = Rect::at(if snap_to_grid { at.round() } else { at }, 0.0, 0.0);
        if let Some(id) = self.new_sprite_at(
            Some(SpriteVisual::new_shape(
                details.colour(),
                shape,
                details.stroke(),
                details.solid(),
            )),
            Some(self.selected_layer),
            at,
        ) {
            self.holding = HeldObject::Anchor(id, 1, 1, at, ephemeral);
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

            self.scene_events(events);
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
            let opt = self.scene.set_sprite_layer(sprite, layer);
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

    pub fn sprite_aura(&mut self, id: Id, colour: scene::Colour) {
        if let Some((visual, rect)) = self.sprite_ref(id).map(|s| {
            let r = s.rect.w.max(s.rect.h) / 2.0 + 1.0;
            let c = s.rect.centre();
            (
                SpriteVisual::Shape {
                    shape: Shape::Ellipse,
                    stroke: Sprite::SOLID_STROKE,
                    solid: true,
                    colour: colour.with_opacity(0.6),
                },
                Rect::at(c - Point::same(r), 2.0 * r, 2.0 * r)
            )
        }) {
            if let Some(aura) = self.new_sprite_at(
                Some(visual),
                Some(self.scene.first_background_layer()),
                rect,
            ) {
                self.clear_selection();
                self.scene.group_sprites(&[id, aura]);
                self.holding = HeldObject::Circle(aura, rect.centre(), false);
            }
        }
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

    fn group_selected(&mut self) {
        let event = self.scene.group_sprites(&self.selected_sprites);
        self.scene_event(event);
    }

    fn ungroup_selected(&mut self) {
        if let Some(&id) = self.selected_sprites.first() {
            if let Some(group) = self.scene.sprite_group(id) {
                let event = self.scene.remove_group(group.id);
                self.scene_event(event);
            }
        }
    }

    pub fn handle_dropdown_event(
        &mut self,
        event: CanvasDropdownEvent,
        details: details::SpriteDetails,
    ) {
        match event {
            CanvasDropdownEvent::Aura => {
                if let Some(id) = self.selected_id() {
                    self.sprite_aura(id, details.colour());
                }
            }
            CanvasDropdownEvent::Clone => {
                if let Some(id) = self.selected_id() {
                    self.clone_sprite(id);
                }
            }
            CanvasDropdownEvent::Delete => {
                if let Some(id) = self.selected_id() {
                    self.remove_sprite(id);
                }
            }
            CanvasDropdownEvent::Group => self.group_selected(),
            CanvasDropdownEvent::Ungroup => self.ungroup_selected(),
            CanvasDropdownEvent::Layer(layer) => {
                if let Some(sprite) = self.selected_id() {
                    self.sprite_layer(sprite, layer)
                }
            }
        }
    }

    pub fn allowed_options(&self) -> &[CanvasDropdownEvent] {
        if self.selected_sprites.len() > 1 {
            if let Some(&id) = self.selected_sprites.first() {
                if self.scene.sprite_group(id).is_some() {
                    return &[];
                } else {
                    return &[CanvasDropdownEvent::Ungroup];
                }
            }
        }
        &[CanvasDropdownEvent::Group, CanvasDropdownEvent::Ungroup]
    }

    pub fn change_fog_brush(&mut self, delta: f32) -> f32 {
        const MIN_FOG_BRUSH: f32 = 0.5;

        if delta.abs() > f32::EPSILON {
            if delta.is_sign_positive() {
                self.fog_brush = MIN_FOG_BRUSH.max(self.fog_brush - 1.0);
            } else {
                self.fog_brush += 1.0;
            }
        }

        self.fog_brush
    }
}
