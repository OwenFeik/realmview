use std::ops::{Add, Sub};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::bridge::{Context, EventType, JsError, Texture, log_float};


#[derive(Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32 
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    fn from_point(point: ScenePoint, w: f32, h: f32) -> Rect {
        Rect { x: point.x, y: point.y, w, h }
    }

    pub fn scaled_from(from: Rect, factor: f32) -> Rect {
        let mut rect  = from.clone();
        rect.scale(factor);
        rect
    }

    pub fn as_floats(&self) -> (f32, f32, f32, f32) {
        (self.x as f32, self.y as f32, self.w as f32, self.h as f32)
    }

    fn scale(&mut self, factor: f32) {
        self.x *= factor;
        self.y *= factor;
        self.w *= factor;
        self.h *= factor;
    }

    fn round(&mut self) {
        self.x = self.x.round();
        self.y = self.y.round();
        self.w = self.w.round();
        self.h = self.h.round();
    
        if self.w < 1.0 {
            self.w = 1.0;
        }

        if self.h < 1.0 {
            self.h = 1.0;
        }
    }

    fn contains_point(&self, point: ScenePoint) -> bool {
        // A negative dimension causes a texture to be flipped. As this is a useful behaviour, negative dimensions on
        // Rects are supported. To that end a different treatment is required for checking if a point is contained.
        // Hence the special cases for negative width and height.

        let in_x;
        if self.w < 0.0 {
            in_x = self.x + self.w <= point.x && point.x <= self.x;
        }
        else {
            in_x = self.x <= point.x && point.x <= self.x + self.w;
        }

        let in_y;
        if self.h < 0.0 {
            in_y = self.y + self.h <= point.y && point.y <= self.y;
        }
        else {
            in_y = self.y <= point.y && point.y <= self.y + self.h;
        }

        in_x && in_y
    }

    fn top_left(&self) -> ScenePoint {
        ScenePoint { x: self.x, y: self.y }
    }
}


pub struct Sprite {
    pub tex: Texture,
    pub rect: Rect,

    // Unique numeric ID, numbered from 1
    id: u32
}

impl Sprite {
    // Distance in scene units from which anchor points (corners, edges) of the sprite can be dragged.
    const ANCHOR_RADIUS: f32 = 10.0;

    pub fn new(tex: Texture) -> Sprite {
        static SPRITE_ID: AtomicU32 = AtomicU32::new(1);

        Sprite {
            tex,
            rect: Rect::new(0.0, 0.0, 1.0, 1.0),
            id: SPRITE_ID.fetch_add(1, Ordering::Relaxed)
        }
    }

    pub fn texture(&self) -> &web_sys::WebGlTexture {
        &self.tex.texture
    }

    fn set_pos(&mut self, ScenePoint { x, y }: ScenePoint) {
        self.rect.x = x;
        self.rect.y = y;
    }

    fn set_size(&mut self, w: f32, h: f32) {
        self.rect.w = w;
        self.rect.h = h;
    }

    fn snap_to_grid(&mut self) {
        self.rect.round();
    }

    fn grab_anchor(&mut self, at: ScenePoint) -> Option<HeldObject> {
        let Rect {x, y, w, h} = self.rect;

        for dx in -1..2 {
            for dy in -1..2 {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let anchor_x = x + (w / 2.0) * (dx + 1) as f32;
                let anchor_y = y + (h / 2.0) * (dy + 1) as f32;

                let delta_x = anchor_x - at.x;
                let delta_y = anchor_y - at.y;

                if (delta_x.powi(2) + delta_y.powi(2)).sqrt() <= Sprite::ANCHOR_RADIUS {
                    return Some(HeldObject::Anchor(self.id, dx, dy));
                }
            }
        }

        None
    }

    fn grab(&mut self, at: ScenePoint) -> HeldObject {
        self.grab_anchor(at).unwrap_or_else(
            || HeldObject::Sprite(self.id, ScenePoint { x: at.x - self.rect.x, y: at.y - self.rect.y })
        )
    }

    fn pos(&mut self) -> ScenePoint {
        ScenePoint { x: self.rect.x, y: self.rect.y }
    }

    fn anchor_point(&mut self, dx: i32, dy: i32) -> ScenePoint {
        let Rect {x, y, w, h} = self.rect;
        ScenePoint { x: x + (w / 2.0) * (dx + 1) as f32, y: y + (h / 2.0) * (dy + 1) as f32 }
    }

    fn update_held_pos(&mut self, holding: HeldObject, at: ScenePoint) {
        match holding {
            HeldObject::Sprite(_, offset) => {
                self.set_pos(at - offset);
            },
            HeldObject::Anchor(_, dx, dy) => {
                let ScenePoint { x: delta_x, y: delta_y } = at - self.anchor_point(dx, dy);
                let x = self.rect.x + (if dx == -1 { delta_x } else { 0.0 });
                let y = self.rect.y + (if dy == -1 { delta_y } else { 0.0 });
                let w = delta_x * (dx as f32) + self.rect.w;
                let h = delta_y * (dy as f32) + self.rect.h;

                self.rect = Rect { x, y, w, h }
            },
            _ => return // Other types aren't sprite-related
        };
    }
}


#[derive(Clone, Copy)]
struct ScenePoint {
    x: f32,
    y: f32
}

impl Add for ScenePoint {
    type Output = ScenePoint;

    fn add(self, rhs: ScenePoint) -> ScenePoint {
        ScenePoint { x: self.x + rhs.x, y: self.y + rhs.y }
    }
}

impl Sub for ScenePoint {
    type Output = ScenePoint;

    fn sub(self, rhs: ScenePoint) -> ScenePoint {
        ScenePoint { x: self.x - rhs.x, y: self.y - rhs.y }
    }
}


#[derive(Clone, Copy)]
enum HeldObject {
    Map(ScenePoint),
    None,
    Sprite(u32, ScenePoint),
    Anchor(u32, i32, i32)
}


pub struct Scene {
    context: Context,

    // (x, y) position of the viewport in scene coordinates.
    viewport: Rect,

    zoom_level: f32,

    // Sprites to be drawn each frame.
    sprites: Vec<Sprite>,

    // ID of the Sprite the user is currently dragging.
    holding: HeldObject,

    // Flag to indicate whether the canvas needs to be rendered (i.e. whether anything has changed).
    redraw_needed: bool
}

impl Scene {
    const BASE_ZOOM_LEVEL: f32 = 50.0;
    const BASE_GRID_SIZE: f32 = 1.0;

    pub fn new() -> Result<Scene, JsError> {
        Ok(
            Scene {
                context: Context::new()?,
                viewport: Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
                zoom_level: Scene::BASE_ZOOM_LEVEL,
                sprites: Vec::new(),
                holding: HeldObject::None,
                redraw_needed: true
            }
        )
    }

    fn grid_size(&self) -> f32 {
        (Scene::BASE_GRID_SIZE * self.zoom_level) as f32
    }

    fn update_viewport(&mut self) -> bool {
        let (w, h) = self.context.viewport_size();
        let w = w as f32;
        let h = h as f32;

        if w != self.viewport.w || h != self.viewport.h {
            self.viewport = Rect { x: self.viewport.x, y: self.viewport.y, w, h};
            return true;
        }

        false
    }

    // Because sprites are added as they are created, they are in the vector ordered by id. Thus they can be binary
    // searched to improve lookup speed to O(log n)
    fn bsearch_sprite(&mut self, id: u32, lo: usize, hi: usize) -> Option<&mut Sprite> {
        if lo == hi {
            return None;
        }

        let m = (lo + hi) / 2;
        match self.sprites.get(m) {
            Some(s) if s.id == id => self.sprites.get_mut(m),
            Some(s) if s.id > id => self.bsearch_sprite(id, m, hi),
            Some(s) if s.id < id => self.bsearch_sprite(id, lo, m),
            _ => None
        }
    }

    fn sprite(&mut self, id: u32) -> Option<&mut Sprite> {
        if id == 0 {
            return None;
        }
        self.bsearch_sprite(id, 0, self.sprites.len())
    }

    fn held_sprite(&mut self) -> Option<&mut Sprite> {
        self.sprite(
            match self.holding {
                HeldObject::Sprite(id, _) => id,
                HeldObject::Anchor(id, _, _) => id,
                _ => return None
            }
        )
    }

    fn sprite_at(&mut self, at: ScenePoint) -> Option<&mut Sprite> {
        // Reversing the iterator atm because the sprites are rendered from the front of the Vec to the back, hence the
        // last Sprite in the Vec is rendered on top, and will be clicked first.
        for sprite in self.sprites.iter_mut().rev() {
            if sprite.rect.contains_point(at) {
                return Some(sprite);
            }
        }

        None
    }

    fn update_held_pos(&mut self, at: ScenePoint) {
        let id = match self.holding {
            HeldObject::Map(ScenePoint { x, y }) => {
                log_float(x - at.x);
                log_float(y - at.y);
        

                self.viewport.x += x - at.x;
                self.viewport.y += y - at.y;
                self.holding = HeldObject::Map(at);
                self.redraw_needed = true;
                return;
            },
            HeldObject::None => return,
            HeldObject::Sprite(id, _) => id,
            HeldObject::Anchor(id, _, _) => id,
        };

        let holding = self.holding;
        self.sprite(id).map(|s| s.update_held_pos(holding, at));

        self.redraw_needed = true;
    }

    fn release_held(&mut self, alt: bool) {
        let held = {
            match self.holding {
                HeldObject::Map(_) => { self.holding = HeldObject::None; return; },
                HeldObject::Sprite(id, _) => id,
                HeldObject::Anchor(id, _, _) => id,
                HeldObject::None => return
            }
        };

        // If alt is held as the sprite is released, the absolute positioning is maintained.
        if !alt {
            self.sprite(held).map(|s| s.snap_to_grid());
        }
        self.holding = HeldObject::None;
        self.redraw_needed = true;
    }

    fn handle_mouse_down(&mut self, at: ScenePoint) {
        self.holding = self.sprite_at(at)
            .map(|s| s.grab(at))
            .unwrap_or(HeldObject::Map(at));
    }

    fn handle_mouse_up(&mut self, at: ScenePoint, alt: bool) {
        self.update_held_pos(at);
        self.release_held(alt);
    }

    fn handle_mouse_move(&mut self, at: ScenePoint) {
        self.update_held_pos(at);
    }

    fn handle_scroll(&mut self, dx: f32, dy: f32, dz: f32, shift: bool, ctrl: bool) {
        const ZOOM_COEFFICIENT: f32 = 0.1 / Scene::BASE_ZOOM_LEVEL;
        const ZOOM_MIN: f32 = Scene::BASE_ZOOM_LEVEL / 5.0;
        const ZOOM_MAX: f32 = Scene::BASE_ZOOM_LEVEL * 5.0;

        // We want shift + scroll to scroll horizontally but browsers (Firefox anyway) only do this when the page is
        // wider than the viewport, which it never is in this case. Thus this check for shift. Likewise for ctrl +
        // scroll and zooming.
        let (dx, dy, dz) = {
            if dx == 0.0 && shift {            
                (dy, 0.0, dz)
            }
            else if dz == 0.0 && ctrl {
                (dx, 0.0, dy)
            }
            else {
                (dx, dy, dz)
            }
        };

        self.viewport.x += dx / self.zoom_level;
        self.viewport.y += dy / self.zoom_level;
        self.zoom_level = (self.zoom_level - ZOOM_COEFFICIENT * dz as f32).clamp(ZOOM_MIN, ZOOM_MAX);

        self.redraw_needed = true;
    }

    fn viewport_to_scene(&self, x: f32, y: f32) -> ScenePoint {
        let p = ScenePoint {
            x: (x / self.zoom_level) + self.viewport.x,
            y: (y / self.zoom_level) + self.viewport.y
        };

        p
    }

    fn process_events(&mut self) {
        let events = match self.context.events() {
            Some(e) => e,
            None => return
        };

        for event in events.iter() {
            let at = self.viewport_to_scene(event.x, event.y);
            match event.event_type {
                EventType::MouseDown => self.handle_mouse_down(at),
                EventType::MouseLeave => self.handle_mouse_up(at, event.alt),
                EventType::MouseMove => self.handle_mouse_move(at),
                EventType::MouseUp => self.handle_mouse_up(at, event.alt),
                EventType::MouseWheel(dx, dy, dz) => self.handle_scroll(dx, dy, dz, event.shift, event.ctrl)
            };
        }
    }

    pub fn animation_frame(&mut self) {
        // We can either process the mouse events and then handle newly loaded images or vice-versa. I choose to process
        // events first because it strikes me as unlikely that the user will have intentionally interacted with a newly
        // loaded image within a frame of it's appearing, and more likely that they instead clicked something that is
        // now behind a newly loaded image.
        self.process_events();
        match self.context.load_queue() {
            Some(mut new_sprites) => {
                self.sprites.append(&mut new_sprites);
                self.redraw_needed = true;
            },
            None => ()
        };

        if self.redraw_needed || self.update_viewport() {
            let vp = self.viewport;
            let grid_size = self.grid_size();

            self.context.clear(vp);
            self.context.draw_grid(vp, grid_size);
            self.context.draw_sprites(vp, &self.sprites, grid_size);    


            let outline = self.held_sprite().map(|s| Rect::scaled_from(s.rect, grid_size));
            
            if let Some(rect) = outline {
                self.context.draw_outline(vp, rect);
            }
        }
        self.redraw_needed = false;
    }
}
