use crate::bridge::{save_scene, timestamp_ms, SaveState};
use crate::dom::menu::{Menu, CanvasDropdownEvent};
use crate::render::Renderer;
use crate::scene::{Point, Rect};
use crate::{
    bridge::{
        event::{Input, Key, KeyboardAction, MouseAction, MouseButton},
        Context, Cursor,
    },
    client::Client,
    interactor::Interactor,
};

#[derive(Clone, Copy, Debug, PartialEq)]
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
            Tool::Fog => Cursor::None,
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

#[derive(Clone, Copy, Debug)]
pub enum DrawTool {
    Circle,
    Cone,
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

    // Menu UI
    menu: Option<Menu>,

    // Measured in scene units (tiles)
    viewport: Rect,

    // Size to render a scene unit, in pixels
    grid_zoom: f32,

    /// Where on the viewport the cursor is. None implies the cursor is not on
    /// the viewport.
    cursor_position: Option<ViewportPoint>,

    /// Whether the left mousebutton is currently being held down.
    mouse_down: Option<bool>,

    /// Whether the control key is currently being held down.
    ctrl_down: bool,

    // Current grab for dragging on the viewport
    grabbed_at: Option<ViewportPoint>,

    // Flag set true whenever something changes
    redraw_needed: bool,

    // Last save time
    last_save: u64,

    // Save progress
    save_state: Option<SaveState>,
}

impl Viewport {
    const BASE_GRID_ZOOM: f32 = 50.0;
    const SAVE_INTERVAL_MS: u64 = 1000 * 5 * 60;

    pub fn new(client: Option<Client>) -> anyhow::Result<Self> {
        let scene = Interactor::new(client);
        let mut vp = Viewport {
            scene,
            context: Context::new()?,
            menu: None,
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
            ctrl_down: false,
            grabbed_at: None,
            redraw_needed: true,
            last_save: timestamp_ms(),
            save_state: None,
        };

        vp.update_viewport();
        vp.centre_viewport();

        Ok(vp)
    }

    pub fn add_menu(&mut self, menu: Menu) {
        self.menu = Some(menu);
        let details = self.scene.get_scene_details();
        self.menu().set_scene_details(details);
        self.menu().set_fog_brush(Interactor::DEFAULT_FOG_BRUSH);
        self.update_layers_menu();
    }

    fn menu(&mut self) -> &mut Menu {
        self.menu.as_mut().unwrap()
    }

    fn update_layers_menu(&mut self) {
        let selected = self.scene.selected_layer();
        let layers = self.scene.layer_info();
        self.menu().set_layer_info(selected, &layers);
    }

    fn update_cursor(&mut self, new: Option<Cursor>) {
        let cursor = if self.grabbed_at.is_some() {
            Cursor::Grabbing
        } else {
            let at = self.scene_point(self.cursor_position.unwrap_or(ViewportPoint { x: 0.0, y: 0.0 }));
            self.scene.cursor(at).override_default(
                self.tool
                    .cursor()
                    .override_default(new.unwrap_or(Cursor::Default)),
            )
        };

        if matches!(cursor, Cursor::None) {
            self.redraw_needed();
        }

        self.context.set_cursor(cursor);
    }

    pub fn set_tool(&mut self, tool: Tool) {
        if self.tool == tool {
            return;
        }

        // As fog cursor is drawn, we'll need to redraw to get rid of it.
        if matches!(self.tool, Tool::Fog) {
            self.redraw_needed();
        }

        if tool.allowed(self.scene.role) {
            self.tool = tool;
            self.update_cursor(None);
            self.menu().update_tool(tool);

            if matches!(self.tool, Tool::Fog) {
                self.enable_fog();
            }
        } else {
            self.set_tool(Tool::Pan);
        }
    }

    pub fn set_draw_tool(&mut self, draw_tool: DrawTool) {
        self.set_tool(Tool::Draw);
        self.menu().set_draw_tool(draw_tool);
    }

    fn enable_fog(&mut self) {
        self.scene
            .scene_details(crate::interactor::details::SceneDetails {
                fog: Some(true),
                ..Default::default()
            });
        let new_details = self.scene.get_scene_details();
        self.menu().set_scene_details(new_details);
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
            self.redraw_needed();
        }
    }

    fn centre_viewport(&mut self) {
        let (w, h) = self.scene.dimensions();
        self.viewport.x = (w as f32 / 2.0 - self.viewport.w / 2.0).round();
        self.viewport.y = (h as f32 / 2.0 - self.viewport.h / 2.0).round();
        self.redraw_needed();
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
                    Tool::Draw => {
                        let menu = self.menu();
                        let draw_details = menu.get_draw_details();
                        let draw_tool = menu.get_draw_tool();
                        self.scene.start_draw(
                            self.scene_point(at),
                            ctrl,
                            alt,
                            draw_details,
                            draw_tool,
                        );
                    }
                    Tool::Pan => self.grab(at),
                    Tool::Select => self.scene.grab(self.scene_point(at), ctrl),
                    _ => (),
                };

                self.menu().hide_dropdown();
                self.mouse_down = Some(true);
            }
            MouseButton::Right => {
                if self.scene.select_at(self.scene_point(at), ctrl) {
                    self.menu().show_dropdown(at);
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

    fn handle_mouse_move(&mut self, at: ViewportPoint, ctrl: bool, shift: bool) {
        let scene_point = self.scene_point(at);
        self.scene.drag(scene_point, shift);
        if let Some(from) = self.grabbed_at {
            self.viewport.x += (from.x - at.x) / self.grid_zoom;
            self.viewport.y += (from.y - at.y) / self.grid_zoom;
            self.grabbed_at = Some(at);
            self.redraw_needed();
        }

        self.update_cursor(Some(self.scene.cursor_at(scene_point, ctrl)));

        if matches!(self.mouse_down, Some(true))
            && matches!(self.tool, Tool::Fog)
            && self.scene.fog().active
        {
            self.scene.set_fog(scene_point, ctrl);
        }
    }

    fn zoom(&mut self, delta: f32, at: Option<ViewportPoint>) {
        const ZOOM_COEFFICIENT: f32 = 3.0 / Viewport::BASE_GRID_ZOOM;
        const ZOOM_MIN: f32 = Viewport::BASE_GRID_ZOOM / 5.0;
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

        self.redraw_needed();
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
        const STROKE_COEFFICIENT: f32 = 0.5;

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
                Tool::Draw => self.menu().handle_stroke_change(delta * STROKE_COEFFICIENT),
                Tool::Fog => {
                    let fog_brush = self.scene.change_fog_brush(delta);
                    self.menu().set_fog_brush(fog_brush);
                }
                _ => {}
            }
        } else {
            self.viewport.y += SCROLL_COEFFICIENT * delta / self.grid_zoom;
        }

        self.redraw_needed();

        // Update the held object details for the scene for the new cursor
        // position.
        self.scene
            .drag(at.scene_point(self.viewport, self.grid_zoom), shift);
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
            self.redraw_needed();
        } else {
            self.scene.move_selection(delta);
        }
    }

    fn set_ctrl_down(&mut self, ctrl: bool) {
        if self.ctrl_down != ctrl {
            self.ctrl_down = ctrl;
            if let Tool::Fog = self.tool {
                self.redraw_needed();
            }
        }
    }

    fn handle_key_down(&mut self, key: Key, ctrl: bool) {
        match key {
            Key::Control => self.set_ctrl_down(true),
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
            Key::E => self.set_draw_tool(DrawTool::Circle),
            Key::F => self.set_draw_tool(DrawTool::Freehand),
            Key::L => self.set_draw_tool(DrawTool::Line),
            Key::O => self.set_draw_tool(DrawTool::Cone),
            Key::Q => self.set_tool(Tool::Select),
            Key::R => self.set_draw_tool(DrawTool::Rectangle),
            Key::S => self.save_scene(),
            Key::V => self.scene.paste(self.target_point()),
            Key::W => self.set_tool(Tool::Fog),
            Key::Y => self.scene.redo(),
            Key::Z => self.scene.undo(),
            k if k.is_arrow() => self.handle_arrow_key_down(key, ctrl),
            _ => {}
        }
    }

    fn handle_key_up(&mut self, key: Key) {
        if let Key::Control = key {
            self.set_ctrl_down(false);
        }
    }

    fn handle_cursor(&mut self, at: ViewportPoint) {
        // As fog cursor is drawn through the renderer, we need to re-render
        // when the cursor moves if the active tool is the fog brush.
        if matches!(self.tool, Tool::Fog) {
            if self.cursor_position.is_none() {
                self.redraw_needed();
            } else {
                let pos = self.cursor_position.unwrap();
                if (pos.x - at.x).abs() >= f32::EPSILON || (pos.y - at.y).abs() >= f32::EPSILON {
                    self.redraw_needed();
                }
            }
        }

        self.cursor_position = Some(at);
    }

    fn process_ui_events(&mut self) {
        let menu = self.menu();
        if let Some(event) = menu.dropdown_event() {
            let draw_details = menu.get_draw_details();
            if matches!(event, CanvasDropdownEvent::Aura) {
                self.set_tool(Tool::Select);
            }
            self.scene.handle_dropdown_event(event, draw_details);
        }

        let events = match self.context.events() {
            Some(e) => e,
            None => return,
        };

        for event in &events {
            self.set_ctrl_down(event.ctrl);
            match event.input {
                Input::Mouse(at, MouseAction::Down, button) => {
                    self.handle_cursor(at);
                    self.handle_mouse_down(at, button, event.ctrl, event.alt)
                }
                Input::Mouse(at, MouseAction::Enter, _) => {
                    self.handle_cursor(at);
                }
                Input::Mouse(_, MouseAction::Leave, button) => {
                    self.cursor_position = None;
                    self.handle_mouse_up(button, event.alt, event.ctrl)
                }
                Input::Mouse(at, MouseAction::Move, _) => {
                    self.handle_cursor(at);
                    self.handle_mouse_move(at, event.ctrl, event.shift)
                }
                Input::Mouse(at, MouseAction::Up, button) => {
                    self.handle_cursor(at);
                    self.handle_mouse_up(button, event.alt, event.ctrl)
                }
                Input::Mouse(at, MouseAction::Wheel(delta), _) => {
                    self.handle_cursor(at);
                    self.handle_scroll(at, delta, event.shift, event.ctrl, event.alt)
                }
                Input::Keyboard(KeyboardAction::Down, key) => self.handle_key_down(key, event.ctrl),
                Input::Keyboard(KeyboardAction::Up, key) => self.handle_key_up(key),
            };
        }
    }

    fn redraw_needed(&mut self) {
        self.redraw_needed = true;
    }

    fn redraw(&mut self) {
        let vp = crate::render::ViewInfo::new(
            Rect::scaled_from(self.viewport, self.grid_zoom),
            self.grid_zoom,
        );

        let fog_brush_outline = self
            .cursor_position
            .map(|at| self.scene_point(at))
            .map(|at| {
                let r = self.scene.get_fog_brush();
                Rect::at(at - Point::same(r), r * 2.0, r * 2.0)
            });
        let renderer = self.context.renderer();

        renderer.clear(vp);
        renderer.draw_scene(vp, self.scene.scene());

        if self.scene.fog().active {
            renderer.draw_fog(vp, self.scene.fog(), self.scene.role.editor());
        }

        renderer.draw_outlines(vp, &self.scene.selections());

        for (at, measurement) in self.scene.active_measurements() {
            let feet = (measurement * 5.).round();
            renderer.draw_text(vp, at, &format!("{feet}ft"));
        }

        if matches!(self.tool, Tool::Fog) && let Some(position) = fog_brush_outline {
            renderer.draw_outline(
                vp,
                position,
                scene::Shape::Ellipse,
                (if self.ctrl_down { scene::Colour::RED } else { scene::Colour::GREEN }).with_opacity(0.6),
            )
        }
    }

    pub fn animation_frame(&mut self) {
        // Handle incoming input events, server events and viewport changes.
        self.process_ui_events();
        if let Some((list, scene)) = self.scene.process_server_events() {
            self.set_scene_list(list);
            self.menu().set_scene(Some(scene));
        }
        self.update_viewport();

        // Redraw the scene if required.
        if self.redraw_needed
            || self.context.load_texture_queue()
            || self.scene.changes.handle_sprite_change()
        {
            self.redraw();
            self.redraw_needed = false;
        }

        // Handle layer changes by updating layers menu.
        if self.scene.changes.handle_layer_change() {
            self.update_layers_menu();
        }

        // Handle selection changes by updating sprite menu.
        if self.scene.changes.handle_selected_change() {
            let details = self.scene.selected_details();
            self.menu().set_sprite_info(details);
            let has_selection = self.scene.has_selection();
            self.menu().update_selection(has_selection);
        }

        // Handle role changes if any by updating visible tools.
        if self.scene.changes.handle_role_change() {
            let new_role = self.scene.role;
            self.menu().update_role(new_role);
        }

        // Save the scene every save interval, as required.
        let now = timestamp_ms();
        if now.saturating_sub(self.last_save) >= Self::SAVE_INTERVAL_MS {
            self.save_scene();
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

    pub fn save_scene(&mut self) {
        if self.scene.save_required() {
            if let Some(scene_key) = self.scene.scene_key() {
                self.save_state = save_scene(&scene_key, self.scene.export()).ok();
                self.last_save = timestamp_ms();
                self.scene.save_done();
            }
        }
    }

    pub fn replace_scene(&mut self, scene: scene::Scene) {
        self.menu()
            .set_scene_details(crate::interactor::details::SceneDetails::from(&scene));
        self.scene.replace_scene(scene);
    }

    pub fn set_scene_list(&mut self, scenes: Vec<(String, String)>) {
        self.menu().set_scene_list(scenes);
        let scene_key = self.scene.scene_key();
        self.menu().set_scene(scene_key);
    }
}
