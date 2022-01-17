use std::sync::atomic::{AtomicU32, Ordering};
use std::ops::{Add, Sub};

use crate::bridge::{Context, EventType, JsError, Texture};


#[derive(Clone, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32 
}

impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }

    fn from_point(point: ScenePoint, w: i32, h: i32) -> Rect {
        Rect { x: point.x, y: point.y, w, h }
    }

    pub fn scaled_from(from: &Rect, factor: i32) -> Rect {
        let mut rect  = from.clone();
        rect.scale(factor);
        rect
    }

    pub fn scaled_from_float(from: &Rect, factor: f32) -> Rect {
        let mut rect = from.clone();
        rect.scale_float(factor);
        rect
    }

    fn scale(&mut self, factor: i32) {
        self.x *= factor;
        self.y *= factor;
        self.w *= factor;
        self.h *= factor;
    }

    fn scale_float(&mut self, factor: f32) {
        self.x = (self.x as f32 * factor).round() as i32;
        self.y = (self.y as f32 * factor).round() as i32;
        self.w = (self.w as f32 * factor).round() as i32;
        self.h = (self.h as f32 * factor).round() as i32;
    }

    fn contains_point(&self, point: ScenePoint) -> bool {
        self.x <= point.x
        && self.y <= point.y
        && point.x <= self.x + self.w
        && point.y <= self.y + self.h
    }
}


// Sprites can be position either on the grid, using tile indexing, or absolutely within the scene, using scene
// coordinates.
enum Positioning {
    Absolute,
    Tile
}


pub struct Sprite {
    pub tex: Texture,
    pub rect: Rect,

    // Unique numeric ID, numbered from 1
    positioning: Positioning,
    id: u32
}

impl Sprite {
    pub fn new(tex: Texture) -> Sprite {
        static SPRITE_ID: AtomicU32 = AtomicU32::new(1);

        Sprite {
            tex,
            rect: Rect::new(0, 0, 1, 1),
            positioning: Positioning::Tile,
            id: SPRITE_ID.fetch_add(1, Ordering::Relaxed)
        }
    }

    pub fn texture(&self) -> &web_sys::WebGlTexture {
        &self.tex.texture
    }

    fn x(&self, grid_size: i32) -> i32 {
        match self.positioning {
            Positioning::Absolute => self.rect.x,
            Positioning::Tile => self.rect.x * grid_size
        }
    }

    fn y(&self, grid_size: i32) -> i32 {
        match self.positioning {
            Positioning::Absolute => self.rect.y,
            Positioning::Tile => self.rect.y * grid_size
        }
    }

    pub fn absolute_rect(&self, grid_size: i32) -> Rect {
        match self.positioning {
            Positioning::Absolute => self.rect.clone(),
            Positioning::Tile => Rect::scaled_from(&self.rect, grid_size)
        }
    }

    fn set_positioning(&mut self, positioning: Positioning) {
        self.positioning = positioning;
    }

    fn set_positioning_opt(&mut self, positioning: Option<Positioning>) {
        match positioning {
            Some(p) => self.set_positioning(p),
            None => return
        };
    }

    fn set_pos(&mut self, ScenePoint { x, y }: ScenePoint) {
        self.rect.x = x;
        self.rect.y = y;
    }

    fn set_size(&mut self, w: i32, h: i32) {
        self.rect.w = w;
        self.rect.h = h;
    }

    fn tile_to_absolute(&mut self, grid_size: i32) {
        match self.positioning {
            Positioning::Absolute => return,
            Positioning::Tile => {
                self.set_positioning(Positioning::Absolute);
                self.rect.scale(grid_size);
            }
        };
    }

    fn absolute_to_tile(&mut self, grid_size: i32) {
        match self.positioning {
            Positioning::Absolute => {
                self.set_positioning(Positioning::Tile);
                self.rect.scale_float(1.0 / grid_size as f32);
            },
            Positioning::Tile => return
        };
    }

    fn grab(&mut self, at: ScenePoint, grid_size: i32) -> HeldObject {
        self.tile_to_absolute(grid_size);
        HeldObject::Sprite(self.id, ScenePoint { x: at.x - self.x(grid_size), y: at.y - self.y(grid_size) })
    } 
}


#[derive(Clone, Copy)]
struct ScenePoint {
    x: i32,
    y: i32
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
struct ViewportPoint {
    x: i32,
    y: i32
}


impl ViewportPoint {
    fn apply_offset(&self, offset: ScenePoint) -> ScenePoint {
        ScenePoint { x: self.x + offset.x, y: self.y + offset.y }
    }
}


enum HeldObject {
    Map(ViewportPoint),
    None,
    Sprite(u32, ScenePoint)
}


pub struct Scene {
    context: Context,

    // (x, y) position of the viewport in scene coordinates.
    viewport_offset: ScenePoint,

    // Sprites to be drawn each frame.
    sprites: Vec<Sprite>,

    // ID of the Sprite the user is currently dragging.
    holding: HeldObject,

    // Flag to indicate whether the canvas needs to be rendered (i.e. whether anything has changed).
    redraw_needed: bool
}

impl Scene {
    pub fn new() -> Result<Scene, JsError> {
        Ok(
            Scene {
                context: Context::new()?,
                viewport_offset: ScenePoint { x: 0, y: 0 },
                sprites: Vec::new(),
                holding: HeldObject::None,
                redraw_needed: true
            }
        )
    }

    fn grid_size(&self) -> i32 {
        return 50;
    }

    fn viewport(&self) -> Rect {
        let (w, h) = self.context.viewport_size();
        Rect::from_point(self.viewport_offset, w as i32, h as i32)
    }

    fn sprite(&mut self, id: u32) -> Option<&mut Sprite> {
        for sprite in self.sprites.iter_mut() {
            if sprite.id == id {
                return Some(sprite);
            }
        }

        None
    }

    fn sprite_at(&mut self, at: ScenePoint) -> Option<&mut Sprite> {
        let grid_size = self.grid_size();
        
        // Reversing the iterator atm because the sprites are rendered from the front of the Vec to the back, hence the
        // last Sprite in the Vec is rendered on top, and will be clicked first.
        for sprite in self.sprites.iter_mut().rev() {
            if sprite.absolute_rect(grid_size).contains_point(at) {
                return Some(sprite);
            }
        }

        None
    }

    fn update_held_pos(&mut self, pos: ViewportPoint) {
        match self.holding {
            HeldObject::Map(ViewportPoint { x, y }) => {
                self.viewport_offset = ScenePoint {
                    x: self.viewport_offset.x + x - pos.x,
                    y: self.viewport_offset.y + y - pos.y
                };
                self.holding = HeldObject::Map(pos);
            },
            HeldObject::Sprite(id, held_at) => {
                let at = pos.apply_offset(self.viewport_offset);

                self.sprite(id).map(|s| s.set_pos(at - held_at));
            },
            HeldObject::None => return
        };

        self.redraw_needed = true;
    }

    fn release_held(&mut self) {
        let grid_size = self.grid_size();

        let held = {
            match self.holding {
                HeldObject::Map(_) => { self.holding = HeldObject::None; return; },
                HeldObject::Sprite(id, _) => id,
                HeldObject::None => return
            }
        };

        self.sprite(held).map(|s| s.absolute_to_tile(grid_size));
        self.holding = HeldObject::None;
    }

    fn handle_mouse_down(&mut self, at: ViewportPoint) {
        let scene_point = at.apply_offset(self.viewport_offset);
        let grid_size = self.grid_size();
        self.holding = self.sprite_at(scene_point)
            .map(|s| s.grab(scene_point, grid_size))
            .unwrap_or(HeldObject::Map(at));
    }

    fn handle_mouse_up(&mut self, at: ViewportPoint) {
        self.update_held_pos(at);
        self.release_held();
    }

    fn handle_mouse_move(&mut self, at: ViewportPoint) {
        self.update_held_pos(at);
    }

    fn process_events(&mut self) {
        let events = match self.context.events() {
            Some(e) => e,
            None => return
        };

        for event in events.iter() {
            let at = ViewportPoint { x: event.x, y: event.y };
            match event.event_type {
                EventType::MouseDown => self.handle_mouse_down(at),
                EventType::MouseUp => self.handle_mouse_up(at),
                EventType::MouseMove => self.handle_mouse_move(at)
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

        if self.redraw_needed {
            self.context.render(self.viewport(), &self.sprites, self.grid_size());
        }
        self.redraw_needed = false;
    }
}
