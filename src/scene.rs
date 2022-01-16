use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

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

    fn contains_point(&self, x: i32, y: i32) -> bool {
        self.x <= x
        && self.y <= y
        && x <= self.x + self.w
        && y <= self.y + self.h
    }
}


struct HeldSprite {
    sprite: u32,
    dx: i32,
    dy: i32
}


// Sprites can be position either on the grid, using tile indexing, or absolutely within the scene, using scene
// coordinates.
pub enum Positioning {
    Absolute,
    Tile
}


pub struct Sprite {
    pub tex: Texture,
    pub positioning: Positioning,
    pub rect: Rect,

    // Unique numeric ID, numbered from 1
    id: u32
}


impl Sprite {
    pub fn new(tex: Texture) -> Sprite {
        static SPRITE_ID: AtomicU32 = AtomicU32::new(1);

        Sprite {
            tex,
            positioning: Positioning::Tile,
            rect: Rect::new(0, 0, 1, 1),
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

    fn set_pos(&mut self, x: i32, y: i32) {
        self.rect.x = x;
        self.rect.y = y;
    }

    fn set_size(&mut self, w: i32, h: i32) {
        self.rect.w = w;
        self.rect.h = h;
    }

    fn set_rect(&mut self, rect: Rect, positioning: Option<Positioning>) {
        self.set_positioning_opt(positioning);
        self.set_pos(rect.x, rect.y);
        self.set_size(rect.w, rect.h);
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

    fn grab(&mut self, x: i32, y: i32, grid_size: i32) -> HeldSprite {
        self.tile_to_absolute(grid_size);
        HeldSprite {
            sprite: self.id,
            dx: x - self.x(grid_size),
            dy: y - self.y(grid_size)
        }
    } 
}


pub struct Scene {
    context: Context,

    // Sprites to be drawn each frame.
    sprites: Vec<Sprite>,

    // ID of the Sprite the user is currently dragging
    holding: Option<HeldSprite>,

    // Flag to indicate whether the canvas needs to be rendered (i.e. whether anything has changed).
    redraw_needed: bool
}


impl Scene {
    pub fn new() -> Result<Scene, JsError> {
        Ok(
            Scene {
                context: Context::new()?,
                sprites: Vec::new(),
                holding: None,
                redraw_needed: true
            }
        )
    }

    fn grid_size(&self) -> i32 {
        return 50;
    }

    fn sprite(&mut self, id: u32) -> Option<&mut Sprite> {
        for sprite in self.sprites.iter_mut() {
            if sprite.id == id {
                return Some(sprite);
            }
        }

        None
    }

    fn sprite_at(&mut self, x: i32, y: i32) -> Option<&mut Sprite> {
        let grid_size = self.grid_size();
        
        // Reversing the iterator atm because the sprites are rendered from the front of the Vec to the back, hence the
        // last Sprite in the Vec is rendered on top, and will be clicked first.
        for sprite in self.sprites.iter_mut().rev() {
            if sprite.absolute_rect(grid_size).contains_point(x, y) {
                return Some(sprite);
            }
        }

        None
    }

    fn update_held_pos(&mut self, x: i32, y: i32) {
        let (held, dx, dy) = {
            match &self.holding {
                Some(h) => (h.sprite, h.dx, h.dy),
                None => return
            }
        };

        self.sprite(held).map(|s| s.set_pos(x - dx, y - dy));

        self.redraw_needed = true;
    }

    fn release_held(&mut self) {
        let grid_size = self.grid_size();

        let held = {
            match &self.holding {
                Some(h) => h.sprite,
                None => return
            }
        };

        self.sprite(held).map(|s| s.absolute_to_tile(grid_size));
        self.holding = None;
    }

    fn handle_mouse_down(&mut self, x: i32, y: i32) {
        let grid_size = self.grid_size();
        self.holding = self.sprite_at(x, y).map(|s| s.grab(x, y, grid_size));
    }

    fn handle_mouse_up(&mut self, x: i32, y: i32) {
        self.update_held_pos(x, y);
        self.release_held();
    }

    fn handle_mouse_move(&mut self, x: i32, y: i32) {
        self.update_held_pos(x, y);
    }

    fn process_events(&mut self) {
        let events = match self.context.events() {
            Some(e) => e,
            None => return
        };

        for event in events.iter() {
            match event.event_type {
                EventType::MouseDown => self.handle_mouse_down(event.x, event.y),
                EventType::MouseUp => self.handle_mouse_up(event.x, event.y),
                EventType::MouseMove => self.handle_mouse_move(event.x, event.y)
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
            self.context.render(&self.sprites, self.grid_size());
        }
        self.redraw_needed = false;
    }
}
