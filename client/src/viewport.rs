use crate::dom::dropdown::Dropdown;
use crate::dom::menu::SceneMenu;
use crate::scene::{Point, Rect};
use crate::{
    bridge::{
        clear_selected_sprite,
        event::{Input, Key, KeyboardAction, MouseAction, MouseButton},
        set_selected_sprite, update_layers_list, Context, Cursor,
    },
    client::Client,
    interactor::Interactor,
};

#[derive(Clone, Copy, Debug, serde_derive::Deserialize, serde_derive::Serialize)]
pub enum Tool {
    Draw,
    Fog,
    Pan,
    Select,
}

impl Tool {
    fn cursor(&self) -> Cursor {
        match self {
            Tool::Draw => Cursor::Crosshair,
            Tool::Fog => Cursor::Crosshair,
            Tool::Pan => Cursor::Grab,
            Tool::Select => Cursor::Default,
        }
    }

    fn allowed(&self, role: scene::perms::Role) -> bool {
        match self {
            Self::Fog => role.editor(),
            Self::Pan => true,
            _ => role.player(),
        }
    }
}

#[derive(serde_derive::Serialize)]
enum DrawTool {
    Ellipse,
    Freehand,
    Line,
    Rectangle,
}

#[derive(Clone, Copy, Debug)]
pub struct ViewportPoint {
    pub x: f32,
    pub y: f32,
}

impl ViewportPoint {
    pub fn new(x: i32, y: i32) -> Self {
        ViewportPoint {
            x: x as f32,
            y: y as f32,
        }
    }

    fn scene_point(&self, viewport: Rect, grid_zoom: f32) -> Point {
        Point::new(
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

    // Canvas context menu
    dropdown: Dropdown,

    // Scene details menu
    scene_menu: SceneMenu,

    // Measured in scene units (tiles)
    viewport: Rect,

    // Size to render a scene unit, in pixels
    grid_zoom: f32,

    /// Where on the viewport the cursor is. None implies the cursor is not on
    /// the viewport.
    cursor_position: Option<ViewportPoint>,

    /// Whether the left mousebutton is currently being held down.
    mouse_down: Option<bool>,

    // Current grab for dragging on the viewport
    grabbed_at: Option<ViewportPoint>,

    // Flag set true whenever something changes
    redraw_needed: bool,
}

impl Viewport {
    const BASE_GRID_ZOOM: f32 = 50.0;

    pub fn new(client: Option<Client>) -> anyhow::Result<Self> {
        let scene = Interactor::new(client);
        let details = scene.get_scene_details();
        let mut vp = Viewport {
            scene,
            context: Context::new()?,
            dropdown: Dropdown::new(),
            scene_menu: SceneMenu::new(details, Interactor::DEFAULT_FOG_BRUSH),
            tool: Tool::Select,
            viewport: Rect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
            grid_zoom: Viewport::BASE_GRID_ZOOM,
            cursor_position: None,
            mouse_down: None,
            grabbed_at: None,
            redraw_needed: true,
        };

        vp.update_viewport();
        vp.centre_viewport();

        Ok(vp)
    }

    fn update_cursor(&self, new: Option<Cursor>) {
        let cursor = if self.grabbed_at.is_some() {
            Cursor::Grabbing
        } else {
            self.scene.cursor().override_default(
                self.tool
                    .cursor()
                    .override_default(new.unwrap_or(Cursor::Default)),
            )
        };

        self.context.set_cursor(cursor);
    }

    pub fn set_tool(&mut self, tool: Tool) {
        if tool.allowed(self.scene.role) {
            crate::bridge::set_active_tool(tool).ok();
            self.tool = tool;
            self.update_cursor(None);
        } else {
            self.set_tool(Tool::Pan);
        }
    }

    fn set_draw_tool(&mut self, draw_tool: DrawTool) {
        self.set_tool(Tool::Draw);

        let mut deets: crate::interactor::details::SpriteDetails = Default::default();
        match draw_tool {
            DrawTool::Ellipse => deets.shape = Some(scene::SpriteShape::Ellipse),
            DrawTool::Freehand => {
                deets.drawing_type = Some(scene::SpriteDrawingType::Freehand);
                deets.cap_end = Some(scene::SpriteCap::Round);
            }
            DrawTool::Line => {
                deets.drawing_type = Some(scene::SpriteDrawingType::Line);
                deets.cap_end = Some(scene::SpriteCap::Arrow);
            }
            DrawTool::Rectangle => deets.shape = Some(scene::SpriteShape::Rectangle),
        }

        self.scene.update_draw_details(deets);
        crate::bridge::set_active_draw_tool(draw_tool).ok();
    }

    fn scene_point(&self, at: ViewportPoint) -> Point {
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
        self.update_cursor(Some(Cursor::Grabbing));
    }

    fn handle_mouse_down(&mut self, at: ViewportPoint, button: MouseButton, ctrl: bool, alt: bool) {
        match button {
            MouseButton::Left => {
                match self.tool {
                    Tool::Draw => self.scene.start_draw(self.scene_point(at), ctrl, alt),
                    Tool::Pan => self.grab(at),
                    Tool::Select => self.scene.grab(self.scene_point(at), ctrl),
                    _ => (),
                };

                self.dropdown.hide();
                self.mouse_down = Some(true);
            }
            MouseButton::Right => {
                if self.scene.select_at(self.scene_point(at), ctrl) {
                    self.dropdown.show(at);
                } else {
                    self.grab(at)
                }
            }
            _ => {}
        };
    }

    fn release_grab(&mut self) {
        self.grabbed_at = None;
        self.update_cursor(None);
    }

    fn handle_mouse_up(&mut self, button: MouseButton, alt: bool, ctrl: bool) {
        match button {
            MouseButton::Left => {
                if let Tool::Pan = self.tool {
                    self.release_grab();
                }
                self.scene.release(alt, ctrl);

                self.mouse_down = Some(false);
            }
            MouseButton::Right => self.release_grab(),
            MouseButton::Middle => self.centre_viewport(),
            _ => {}
        };
    }

    fn handle_mouse_move(&mut self, at: ViewportPoint, ctrl: bool) {
        let scene_point = self.scene_point(at);
        self.scene.drag(scene_point);
        if let Some(from) = self.grabbed_at {
            self.viewport.x += (from.x - at.x) / self.grid_zoom;
            self.viewport.y += (from.y - at.y) / self.grid_zoom;
            self.grabbed_at = Some(at);
            self.redraw_needed = true;
        }

        self.update_cursor(Some(self.scene.cursor_at(scene_point, ctrl)));

        if matches!(self.mouse_down, Some(true)) && matches!(self.tool, Tool::Fog) {
            self.scene.set_fog(scene_point, ctrl);
        }
    }

    fn zoom(&mut self, delta: f32, at: Option<ViewportPoint>) {
        const ZOOM_COEFFICIENT: f32 = 3.0 / Viewport::BASE_GRID_ZOOM;
        const ZOOM_MIN: f32 = Viewport::BASE_GRID_ZOOM / 2.0;
        const ZOOM_MAX: f32 = Viewport::BASE_GRID_ZOOM * 5.0;

        let at = at.unwrap_or_else(|| self.centre());

        // Need to calculate these before changing the zoom level
        let scene_point = at.scene_point(self.viewport, self.grid_zoom);
        let fraction_x = at.x / (self.viewport.w * self.grid_zoom);
        let fraction_y = at.y / (self.viewport.h * self.grid_zoom);

        self.grid_zoom = (self.grid_zoom - delta * ZOOM_COEFFICIENT).clamp(ZOOM_MIN, ZOOM_MAX);
        self.update_viewport();

        // Update viewport such that the mouse is at the same scene
        // coordinate as before zooming.
        self.viewport.x = scene_point.x - self.viewport.w * fraction_x;
        self.viewport.y = scene_point.y - self.viewport.h * fraction_y;

        self.redraw_needed = true;
    }

    fn zoom_in(&mut self) {
        const ZOOM_AMT: f32 = -Viewport::BASE_GRID_ZOOM;
        self.zoom(ZOOM_AMT, None);
    }

    fn zoom_out(&mut self) {
        const ZOOM_AMT: f32 = Viewport::BASE_GRID_ZOOM;
        self.zoom(ZOOM_AMT, None);
    }

    fn handle_scroll(&mut self, at: ViewportPoint, delta: f32, shift: bool, ctrl: bool, alt: bool) {
        const SCROLL_COEFFICIENT: f32 = 0.5;

        // We want shift + scroll to scroll horizontally but browsers (Firefox
        // anyway) only do this when the page is wider than the viewport, which
        // it never is in this case. Thus this check for shift. Likewise for
        // ctrl + scroll and zooming.
        if shift {
            self.viewport.x += SCROLL_COEFFICIENT * delta / self.grid_zoom;
        } else if ctrl {
            self.zoom(delta, Some(at));
        } else if alt {
            match self.tool {
                Tool::Draw => self.scene.change_stroke(delta),
                Tool::Fog => self
                    .scene_menu
                    .set_fog_brush(self.scene.change_fog_brush(delta)),
                _ => {}
            }
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
            Key::Down => Point { x: 0.0, y: 1.0 },
            Key::Left => Point { x: -1.0, y: 0.0 },
            Key::Right => Point { x: 1.0, y: 0.0 },
            Key::Up => Point { x: 0.0, y: -1.0 },
            _ => Point { x: 0.0, y: 0.0 },
        };

        if ctrl || !self.scene.has_selection() {
            self.viewport.translate_in_place(delta);
            self.redraw_needed = true;
        } else {
            self.scene.move_selection(delta);
        }
    }

    fn handle_key_down(&mut self, key: Key, ctrl: bool) {
        match key {
            Key::Delete => self.scene.remove_selection(),
            Key::Escape => {
                self.scene.clear_selection();
                self.set_tool(Tool::Select);
            }
            Key::Plus | Key::Equals => self.zoom_in(),
            Key::Minus | Key::Underscore => self.zoom_out(),
            Key::Space => self.set_tool(Tool::Pan),
            Key::A => {
                self.scene.select_all();
                self.set_tool(Tool::Select);
            }
            Key::C => self.scene.copy(),
            Key::D => self.scene.clear_selection(),
            Key::E => self.set_draw_tool(DrawTool::Ellipse),
            Key::F => self.set_draw_tool(DrawTool::Freehand),
            Key::L => self.set_draw_tool(DrawTool::Line),
            Key::Q => self.set_tool(Tool::Select),
            Key::R => self.set_draw_tool(DrawTool::Rectangle),
            Key::V => self.scene.paste(self.target_point()),
            Key::W => self.set_tool(Tool::Fog),
            Key::Y => self.scene.redo(),
            Key::Z => self.scene.undo(),
            k if k.is_arrow() => self.handle_arrow_key_down(key, ctrl),
            _ => {}
        }
    }

    fn process_ui_events(&mut self) {
        if let Some(event) = self.dropdown.event() {
            self.scene.handle_dropdown_event(event);
        }

        if self.scene_menu.changed() {
            self.scene.scene_details(self.scene_menu.details());
            self.scene.set_fog_brush(self.scene_menu.fog_brush());
        }

        let events = match self.context.events() {
            Some(e) => e,
            None => return,
        };

        for event in &events {
            match event.input {
                Input::Mouse(at, MouseAction::Down, button) => {
                    self.cursor_position = Some(at);
                    self.handle_mouse_down(at, button, event.ctrl, event.alt)
                }
                Input::Mouse(at, MouseAction::Enter, _) => {
                    self.cursor_position = Some(at);
                }
                Input::Mouse(_, MouseAction::Leave, button) => {
                    self.cursor_position = None;
                    self.handle_mouse_up(button, event.alt, event.ctrl)
                }
                Input::Mouse(at, MouseAction::Move, _) => {
                    self.cursor_position = Some(at);
                    self.handle_mouse_move(at, event.ctrl)
                }
                Input::Mouse(at, MouseAction::Up, button) => {
                    self.cursor_position = Some(at);
                    self.handle_mouse_up(button, event.alt, event.ctrl)
                }
                Input::Mouse(at, MouseAction::Wheel(delta), _) => {
                    self.cursor_position = Some(at);
                    self.handle_scroll(at, delta, event.shift, event.ctrl, event.alt)
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

        if self.scene.fog().active {
            self.context.draw_fog(
                vp,
                self.grid_zoom,
                self.scene.fog(),
                self.scene.role.editor(),
            );
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
            self.dropdown.update_layers(self.scene.layers());
            update_layers_list(self.scene.layers());
        }

        if self.scene.changes.handle_selected_change() {
            if let Some(details) = self.scene.selected_details() {
                set_selected_sprite(details);
            } else {
                clear_selected_sprite();
            }
            self.dropdown.update_options(self.scene.allowed_options());
        }
    }

    fn centre(&self) -> ViewportPoint {
        ViewportPoint {
            x: (self.viewport.w / 2.0) * self.grid_zoom,
            y: (self.viewport.h / 2.0) * self.grid_zoom,
        }
    }

    fn target_point(&self) -> Point {
        self.cursor_position
            .map(|p| self.scene_point(p))
            .unwrap_or_else(|| self.centre_tile())
    }

    fn centre_tile(&self) -> Point {
        (self.viewport.top_left()
            + Point {
                x: self.viewport.w / 2.0,
                y: self.viewport.h / 2.0,
            })
        .round()
    }

    pub fn placement_tile(&self) -> Point {
        let centre = self.centre_tile();
        if self.scene.role.editor() {
            centre
        } else {
            let x = centre.x as u32;
            let y = centre.y as u32;
            let nearest = self.scene.fog().nearest_clear(x, y);
            if nearest != (x, y) {
                Point::new(nearest.0 as f32, nearest.1 as f32)
            } else {
                centre
            }
        }
    }
}
