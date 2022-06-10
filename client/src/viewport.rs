use std::sync::atomic::{AtomicI64, Ordering};

use crate::{
    bridge::{Context, EventType, JsError, MouseButton},
    client::Client,
};
use bincode::serialize;
use scene::{
    comms::{ClientEvent, ClientMessage, SceneEvent, SceneEventAck, ServerEvent},
    Id, Layer, Rect, Scene, ScenePoint, Sprite,
};

#[derive(Clone, Copy, Debug)]
pub struct ViewportPoint {
    x: f32,
    y: f32,
}

impl ViewportPoint {
    pub fn new(x: i32, y: i32) -> Self {
        ViewportPoint {
            x: x as f32,
            y: y as f32,
        }
    }

    fn scene_point(&self, viewport: Rect, grid_zoom: f32) -> ScenePoint {
        ScenePoint::new(
            (self.x / grid_zoom) + viewport.x,
            (self.y / grid_zoom) + viewport.y,
        )
    }
}

pub struct Viewport {
    context: Context,
    scene: Scene,

    // Measured in scene units (tiles)
    viewport: Rect,

    // Size to render a scene unit, in pixels
    grid_zoom: f32,

    // Flag set true whenever something changes
    redraw_needed: bool,

    // Position where the viewport is being dragged from
    grabbed_at: Option<ViewportPoint>,

    // Events that this client has sent to the server, awaiting approval.
    issued_events: Vec<ClientMessage>,

    // Wrapper for a potential WebSocket connection with the server.
    client: Option<Client>,
}

impl Viewport {
    const BASE_GRID_ZOOM: f32 = 50.0;

    pub fn new(client: Option<Client>) -> Result<Self, JsError> {
        let mut vp = Viewport {
            context: Context::new()?,
            scene: Scene::new(),
            viewport: Rect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
            grid_zoom: Viewport::BASE_GRID_ZOOM,
            redraw_needed: true,
            grabbed_at: None,
            issued_events: Vec::new(),
            client,
        };

        vp.update_viewport();
        vp.centre_viewport();

        Ok(vp)
    }

    fn update_viewport(&mut self) {
        let (w, h) = self.context.viewport_size();
        let w = w as f32 / self.grid_zoom;
        let h = h as f32 / self.grid_zoom;

        if w != self.viewport.w || h != self.viewport.h {
            self.viewport = Rect {
                x: self.viewport.x,
                y: self.viewport.y,
                w,
                h,
            };
            self.redraw_needed = true;
        }
    }

    fn centre_viewport(&mut self) {
        self.viewport.x = ((self.scene.w / 2) as f32 - self.viewport.w / 2.0).round();
        self.viewport.y = ((self.scene.h / 2) as f32 - self.viewport.h / 2.0).round();
        self.redraw_needed = true;
    }

    fn handle_mouse_down(&mut self, at: ViewportPoint) {
        if !self
            .scene
            .grab(at.scene_point(self.viewport, self.grid_zoom))
        {
            self.grabbed_at = Some(at)
        }
    }

    fn handle_mouse_up(&mut self, _at: ViewportPoint, alt: bool) {
        if !self.scene.holding.is_none() {
            self.redraw_needed = true;
        }

        // Only snap a held sprite to the grid if the user is not holding alt.
        if let Some(scene_event) = self.scene.release_held(!alt) {
            self.client_event(scene_event);
        }
        self.grabbed_at = None;
    }

    fn handle_mouse_move(&mut self, at: ViewportPoint) {
        if !self.scene.holding.is_none() {
            self.redraw_needed = true;
        }

        if let Some(scene_event) = self
            .scene
            .update_held_pos(at.scene_point(self.viewport, self.grid_zoom))
        {
            self.client_event(scene_event);
        }

        if let Some(ViewportPoint { x, y }) = self.grabbed_at {
            self.viewport.x += (x - at.x) / self.grid_zoom;
            self.viewport.y += (y - at.y) / self.grid_zoom;
            self.grabbed_at = Some(at);
            self.redraw_needed = true;
        }
    }

    fn handle_scroll(&mut self, at: ViewportPoint, delta: f32, shift: bool, ctrl: bool) {
        const SCROLL_COEFFICIENT: f32 = 0.5;
        const ZOOM_COEFFICIENT: f32 = 3.0 / Viewport::BASE_GRID_ZOOM;
        const ZOOM_MIN: f32 = Viewport::BASE_GRID_ZOOM / 2.0;
        const ZOOM_MAX: f32 = Viewport::BASE_GRID_ZOOM * 5.0;

        // We want shift + scroll to scroll horizontally but browsers (Firefox
        // anyway) only do this when the page is wider than the viewport, which
        // it never is in this case. Thus this check for shift. Likewise for
        // ctrl + scroll and zooming.
        if shift {
            self.viewport.x += SCROLL_COEFFICIENT * delta / self.grid_zoom;
        } else if ctrl {
            // Need to calculate these before changing the zoom level
            let scene_point = at.scene_point(self.viewport, self.grid_zoom);
            let fraction_x = at.x / (self.viewport.w * self.grid_zoom);
            let fraction_y = at.y / (self.viewport.h * self.grid_zoom);

            // Zoom in
            self.grid_zoom = (self.grid_zoom - ZOOM_COEFFICIENT * delta).clamp(ZOOM_MIN, ZOOM_MAX);
            self.update_viewport();

            // Update viewport such that the mouse is at the same scene
            // coordinate as before zooming.
            self.viewport.x = scene_point.x - self.viewport.w * fraction_x;
            self.viewport.y = scene_point.y - self.viewport.h * fraction_y;
        } else {
            self.viewport.y += SCROLL_COEFFICIENT * delta / self.grid_zoom;
        }

        self.redraw_needed = true;
    }

    fn process_ui_events(&mut self) {
        let events = match self.context.events() {
            Some(e) => e,
            None => return,
        };

        for event in events.iter() {
            if matches!(event.button, MouseButton::Middle) {
                self.centre_viewport();
                continue;
            }

            match event.event_type {
                EventType::MouseDown => self.handle_mouse_down(event.at),
                EventType::MouseLeave => self.handle_mouse_up(event.at, event.alt),
                EventType::MouseMove => self.handle_mouse_move(event.at),
                EventType::MouseUp => self.handle_mouse_up(event.at, event.alt),
                EventType::MouseWheel(delta) => {
                    self.handle_scroll(event.at, delta, event.shift, event.ctrl)
                }
            };
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
                self.scene.apply_event(scene_event, false);
            }
        }
    }

    fn process_server_events(&mut self) {
        if let Some(client) = &self.client {
            let mut events = client.events();
            while let Some(event) = events.pop() {
                self.process_server_event(event);
                self.redraw_needed = true;
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

    fn redraw(&mut self) {
        let vp = Rect::scaled_from(self.viewport, self.grid_zoom);

        self.context.clear(vp);

        let mut background_drawn = false;
        for layer in self.scene.layers.iter() {
            if !background_drawn && layer.z >= 0 {
                self.context
                    .draw_grid(vp, self.scene.w, self.scene.h, self.grid_zoom);
                background_drawn = true;
            }

            if layer.visible {
                self.context
                    .draw_sprites(vp, &layer.sprites, self.grid_zoom);
            }
        }

        if !background_drawn {
            self.context
                .draw_grid(vp, self.scene.w, self.scene.h, self.grid_zoom);
        }

        let outline = self
            .scene
            .held_sprite()
            .map(|s| Rect::scaled_from(s.rect, self.grid_zoom));

        if let Some(rect) = outline {
            self.context.draw_outline(vp, rect);
        }
    }

    pub fn animation_frame(&mut self) {
        self.process_ui_events();
        if self.context.load_texture_queue() {
            self.redraw_needed = true;
        }
        self.process_server_events();

        self.update_viewport();

        if self.redraw_needed {
            self.redraw();
            self.redraw_needed = false;
        }
    }

    pub fn export(&self) -> Vec<u8> {
        match serialize(&self.scene) {
            Ok(v) => v,
            Err(_) => vec![],
        }
    }

    pub fn layers(&self) -> &[Layer] {
        &self.scene.layers
    }

    pub fn new_scene(&mut self, id: Id) {
        if self.scene.id.is_some() {
            self.scene = Scene::new();
            if id != 0 {
                self.scene.project = Some(id);
            }
            self.redraw_needed = true;
        }
    }

    pub fn replace_scene(&mut self, mut new: Scene) {
        new.refresh_local_ids();
        self.scene = new;
        self.redraw_needed = true;
    }

    pub fn new_sprite(&mut self, texture: Id) {
        let opt = self.scene.add_sprite(Sprite::new(texture), 0);
        self.client_option(opt);
        self.redraw_needed = true;
    }

    pub fn rename_layer(&mut self, layer: Id, title: String) {
        let opt = self.scene.rename_layer(layer, title);
        self.client_option(opt);
    }

    pub fn set_layer_visible(&mut self, layer: Id, visible: bool) {
        if let Some(l) = self.scene.layer(layer) {
            let opt = l.set_visible(visible);
            self.client_option(opt);
            self.redraw_needed = true;
        }
    }

    pub fn set_layer_locked(&mut self, layer: Id, locked: bool) {
        if let Some(l) = self.scene.layer(layer) {
            let opt = l.set_locked(locked);
            self.client_option(opt);
        }
    }
}
