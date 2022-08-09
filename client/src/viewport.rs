use crate::{
    bridge::{
        clear_selected_sprite, set_scene_details, set_selected_sprite, sprite_dropdown,
        update_layers_list, Context, Input, JsError, Key, KeyboardAction, MouseAction, MouseButton,
    },
    client::Client,
    interactor::Interactor,
};
use scene::{Rect, ScenePoint, SpriteShape};

pub enum Tool {
    Draw,
    Select,
    Shape(SpriteShape),
}

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
    pub scene: Interactor,

    // Currently active tool
    tool: Tool,

    // WebGL rendering context wrapper
    context: Context,

    // Measured in scene units (tiles)
    viewport: Rect,

    // Size to render a scene unit, in pixels
    grid_zoom: f32,

    // Current grab for dragging on the viewport
    grabbed_at: Option<ViewportPoint>,

    // Flag set true whenever something changes
    redraw_needed: bool,
}

impl Viewport {
    const BASE_GRID_ZOOM: f32 = 50.0;

    pub fn new(client: Option<Client>) -> Result<Self, JsError> {
        let mut vp = Viewport {
            scene: Interactor::new(client),
            context: Context::new()?,
            tool: Tool::Select,
            viewport: Rect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
            grid_zoom: Viewport::BASE_GRID_ZOOM,
            grabbed_at: None,
            redraw_needed: true,
        };

        vp.update_viewport();
        vp.centre_viewport();
        set_scene_details(vp.scene.get_scene_details());

        Ok(vp)
    }

    fn scene_point(&self, at: ViewportPoint) -> ScenePoint {
        at.scene_point(self.viewport, self.grid_zoom)
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
        let scene_size = self.scene.dimensions();
        self.viewport.x = (scene_size.w / 2.0 - self.viewport.w / 2.0).round();
        self.viewport.y = (scene_size.h / 2.0 - self.viewport.h / 2.0).round();
        self.redraw_needed = true;
    }

    fn grab(&mut self, at: ViewportPoint) {
        if self.grabbed_at.is_none() {
            self.grabbed_at = Some(at);
        }
    }

    fn handle_mouse_down(&mut self, at: ViewportPoint, button: MouseButton, ctrl: bool) {
        match button {
            MouseButton::Left => match self.tool {
                Tool::Draw => self.scene.start_draw(self.scene_point(at)),
                Tool::Select => self.scene.grab(self.scene_point(at), ctrl),
                Tool::Shape(shape) => self.scene.new_held_shape(shape, self.scene_point(at)),
            },
            MouseButton::Right => {
                if let Some(id) = self.scene.sprite_at(self.scene_point(at)) {
                    sprite_dropdown(id, at.x, at.y);
                } else {
                    self.grab(at)
                }
            }
            _ => {}
        };
    }

    fn release_grab(&mut self) {
        self.grabbed_at = None;
    }

    fn handle_mouse_up(&mut self, button: MouseButton, alt: bool, ctrl: bool) {
        match button {
            MouseButton::Left => self.scene.release(alt, ctrl),
            MouseButton::Right => self.release_grab(),
            MouseButton::Middle => self.centre_viewport(),
            _ => {}
        };
    }

    fn handle_mouse_move(&mut self, at: ViewportPoint) {
        self.scene
            .drag(at.scene_point(self.viewport, self.grid_zoom));
        if let Some(from) = self.grabbed_at {
            self.viewport.x += (from.x - at.x) / self.grid_zoom;
            self.viewport.y += (from.y - at.y) / self.grid_zoom;
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

        // Update the held object details for the scene for the new cursor
        // position.
        self.scene
            .drag(at.scene_point(self.viewport, self.grid_zoom));
    }

    fn handle_arrow_key_down(&mut self, key: Key, ctrl: bool) {
        let delta = match key {
            Key::Down => ScenePoint { x: 0.0, y: 1.0 },
            Key::Left => ScenePoint { x: -1.0, y: 0.0 },
            Key::Right => ScenePoint { x: 1.0, y: 0.0 },
            Key::Up => ScenePoint { x: 0.0, y: -1.0 },
            _ => ScenePoint { x: 0.0, y: 0.0 },
        };

        if ctrl || !self.scene.has_selection() {
            self.viewport.translate(delta);
            self.redraw_needed = true;
        } else {
            self.scene.move_selection(delta);
        }
    }

    fn handle_key_down(&mut self, key: Key, ctrl: bool) {
        if key.is_arrow() {
            self.handle_arrow_key_down(key, ctrl);
            return;
        }

        match key {
            Key::Delete => self.scene.remove_sprite(Interactor::SELECTION_ID),
            Key::Escape => self.scene.clear_selection(),
            Key::Y => self.scene.redo(),
            Key::Z => self.scene.undo(),
            _ => {}
        }
    }

    fn process_ui_events(&mut self) {
        let events = match self.context.events() {
            Some(e) => e,
            None => return,
        };

        for event in &events {
            match event.input {
                Input::Mouse(at, MouseAction::Down, button) => {
                    self.handle_mouse_down(at, button, event.ctrl)
                }
                Input::Mouse(_, MouseAction::Leave, button) => {
                    self.handle_mouse_up(button, event.alt, event.ctrl)
                }
                Input::Mouse(at, MouseAction::Move, _) => self.handle_mouse_move(at),
                Input::Mouse(_, MouseAction::Up, button) => {
                    self.handle_mouse_up(button, event.alt, event.ctrl)
                }
                Input::Mouse(at, MouseAction::Wheel(delta), _) => {
                    self.handle_scroll(at, delta, event.shift, event.ctrl)
                }
                Input::Keyboard(KeyboardAction::Down, key) => self.handle_key_down(key, event.ctrl),
                Input::Keyboard(KeyboardAction::Up, _) => (),
            };
        }
    }

    fn redraw(&mut self) {
        let vp = Rect::scaled_from(self.viewport, self.grid_zoom);

        self.context.clear(vp);

        let mut background_drawn = false;
        for layer in self.scene.layers().iter().rev() {
            if !background_drawn && layer.z >= 0 {
                self.context
                    .draw_grid(vp, self.scene.dimensions(), self.grid_zoom);
                background_drawn = true;
            }

            if layer.visible {
                self.context
                    .draw_sprites(vp, &layer.sprites, self.grid_zoom);
            }
        }

        if !background_drawn {
            self.context
                .draw_grid(vp, self.scene.dimensions(), self.grid_zoom);
        }

        for rect in self.scene.selections() {
            self.context
                .draw_outline(vp, Rect::scaled_from(rect, self.grid_zoom));
        }
    }

    pub fn animation_frame(&mut self) {
        self.process_ui_events();
        self.scene.process_server_events();
        self.update_viewport();
        if self.redraw_needed
            || self.context.load_texture_queue()
            || self.scene.changes.handle_sprite_change()
        {
            self.redraw();
            self.redraw_needed = false;
        }

        if self.scene.changes.handle_layer_change() {
            update_layers_list(self.scene.layers());
        }

        if self.scene.changes.handle_selected_change() {
            if let Some(details) = self.scene.selected_details() {
                set_selected_sprite(details);
            } else {
                clear_selected_sprite();
            }
        }
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
    }

    pub fn centre_tile(&self) -> ScenePoint {
        (self.viewport.top_left()
            + ScenePoint {
                x: self.viewport.w / 2.0,
                y: self.viewport.h / 2.0,
            })
        .round()
    }
}
