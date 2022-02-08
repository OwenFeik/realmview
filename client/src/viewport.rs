use std::sync::atomic::{AtomicI64, Ordering};

use crate::{
    bridge::{Context, EventType, JsError},
    client::Client,
};
use scene::{
    comms::{ClientEvent, SceneEvent, ServerEvent},
    Id, Rect, Scene, ScenePoint,
};

#[derive(Clone, Copy)]
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
    issued_events: Vec<ClientEvent>,

    // Wrapper for a potential WebSocket connection with the server.
    client: Option<Client>,
}

impl Viewport {
    const BASE_GRID_ZOOM: f32 = 50.0;

    pub fn new(client: Option<Client>) -> Result<Self, JsError> {
        Ok(Viewport {
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
        })
    }

    fn update_viewport(&mut self) -> bool {
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
            return true;
        }

        false
    }

    fn handle_mouse_down(&mut self, at: ViewportPoint) {
        if !self
            .scene
            .grab(at.scene_point(self.viewport, self.grid_zoom), true)
        {
            self.grabbed_at = Some(at)
        }
    }

    fn handle_mouse_up(&mut self, _at: ViewportPoint, alt: bool) {
        // Only snap a held sprite to the grid if the user is not holding alt.
        if let Some(scene_event) = self.scene.release_held(!alt) {
            self.client_event(scene_event);
            self.redraw_needed = true;
        }
        self.grabbed_at = None;
    }

    fn handle_mouse_move(&mut self, at: ViewportPoint) {
        if let Some(scene_event) = self
            .scene
            .update_held_pos(at.scene_point(self.viewport, self.grid_zoom))
        {
            self.client_event(scene_event);
            self.redraw_needed = true;
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
        if let Some(c) = self.issued_events.iter().find(|c| c.id == id) {
            self.scene.unwind_event(&c.scene_event);
        }
        self.issued_events.retain(|c| c.id != id);
    }

    fn process_server_event(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::Approval(id) => self.approve_event(id),
            ServerEvent::Rejection(id) => self.unwind_event(id),
            ServerEvent::SceneChange(scene_event, _url) => {
                // TODO load texture from supplied url

                self.scene.apply_event(&scene_event, true);
            }
            ServerEvent::CanonicalId(local_id, canonical_id) => {
                self.scene.set_canonical_id(local_id, canonical_id)
            }
        }
    }

    fn process_server_events(&mut self) {
        if let Some(client) = &self.client {
            let mut events = client.recv_events();
            while let Some(event) = events.pop() {
                self.process_server_event(event);
            }
        }
    }

    fn client_event(&mut self, scene_event: SceneEvent) {
        static EVENT_ID: AtomicI64 = AtomicI64::new(1);

        // nop unless we actually have a connection.
        if let Some(client) = &self.client {
            let event = ClientEvent {
                id: EVENT_ID.fetch_add(1, Ordering::Relaxed),
                scene_event,
            };
            client.send_event(&event);
            self.issued_events.push(event);
        }
    }

    pub fn animation_frame(&mut self) {
        // We can either process the mouse events and then handle newly loaded
        // images or vice-versa. I choose to process events first because it
        // strikes me as unlikely that the user will have intentionally
        // interacted with a newly loaded image within a frame of it's
        // appearing, and more likely that they instead clicked something that
        // is now behind a newly loaded image.
        self.process_ui_events();
        if let Some(mut new_sprites) = self.context.load_queue() {
            self.scene.add_tokens(&mut new_sprites);
            self.redraw_needed = true;
        };

        self.process_server_events();

        if self.redraw_needed || self.update_viewport() {
            let vp = Rect::scaled_from(self.viewport, self.grid_zoom);

            self.context.clear(vp);
            self.context
                .draw_sprites(vp, self.scene.scenery(), self.grid_zoom);
            self.context.draw_grid(vp, self.grid_zoom);
            self.context
                .draw_sprites(vp, self.scene.tokens(), self.grid_zoom);

            let outline = self
                .scene
                .held_sprite()
                .map(|s| Rect::scaled_from(s.rect, self.grid_zoom));

            if let Some(rect) = outline {
                self.context.draw_outline(vp, rect);
            }
        }
        self.redraw_needed = false;
    }
}
