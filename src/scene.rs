use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use crate::bridge::{Context, EventType, JsError, Texture};


#[derive(PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32 
}


struct HeldSprite {
    sprite: u32,
    dx: i32,
    dy: i32
}


pub struct Sprite {
    pub tex: Texture,
    pub x: i32,
    pub y: i32,

    // Unique numeric ID, numbered from 1
    id: u32
}


impl Sprite {
    pub fn new(tex: Texture) -> Sprite {
        static SPRITE_ID: AtomicU32 = AtomicU32::new(1);

        Sprite { tex, x: 0, y: 0, id: SPRITE_ID.fetch_add(1, Ordering::Relaxed) }
    }

    fn new_at(texture: Texture, x: i32, y: i32) -> Sprite {
        let mut sprite = Sprite::new(texture);
        sprite.set_pos(x, y);
        sprite
    }

    pub fn texture(&self) -> &web_sys::WebGlTexture {
        &self.tex.texture
    }

    pub fn position(&self) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            w: self.tex.width as i32,
            h: self.tex.height as i32
        }
    }

    fn set_pos(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    fn touches_point(&self, x: i32, y: i32) -> bool {
        self.x <= x
        && self.y <= y
        && x <= self.x + self.tex.width as i32
        && y <= self.y + self.tex.height as i32
    }

    fn grab(&self, x: i32, y: i32) -> HeldSprite {
        HeldSprite {
            sprite: self.id,
            dx: x - self.x,
            dy: y - self.y
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

    fn sprite(&mut self, id: u32) -> Option<&mut Sprite> {
        for sprite in self.sprites.iter_mut() {
            if sprite.id == id {
                return Some(sprite);
            }
        }

        None
    }

    fn sprite_at(&self, x: i32, y: i32) -> Option<&Sprite> {
        // Reversing the iterator atm because the sprites are rendered from the front of the Vec to the back, hence the
        // last Sprite in the Vec is rendered on top, and will be clicked first.
        for sprite in self.sprites.iter().rev() {
            if sprite.touches_point(x, y) {
                return Some(&sprite);
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

    fn handle_mouse_down(&mut self, x: i32, y: i32) {
        self.holding = self.sprite_at(x, y).map(|s| s.grab(x, y));
    }

    fn handle_mouse_up(&mut self, x: i32, y: i32) {
        self.update_held_pos(x, y);
        self.holding = None;
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
            self.context.render(&self.sprites);
        }
        self.redraw_needed = false;
    }
}
