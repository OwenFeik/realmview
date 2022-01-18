use std::ops::{Add, Sub};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::bridge::{Context, EventType, JsError, Texture};


#[derive(Clone, Copy, PartialEq)]
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

    pub fn scaled_from(from: Rect, factor: i32) -> Rect {
        let mut rect  = from.clone();
        rect.scale(factor);
        rect
    }

    pub fn scaled_from_float(from: Rect, factor: f32) -> Rect {
        let mut rect = from.clone();
        rect.scale_float(factor);
        rect
    }

    pub fn as_floats(&self) -> (f32, f32, f32, f32) {
        (self.x as f32, self.y as f32, self.w as f32, self.h as f32)
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
        // A negative dimension causes a texture to be flipped. As this is a useful behaviour, negative dimensions on
        // Rects are supported. To that end a different treatment is required for checking if a point is contained.
        // Hence the special cases for negative width and height.

        let in_x;
        if self.w < 0 {
            in_x = self.x + self.w <= point.x && point.x <= self.x;
        }
        else {
            in_x = self.x <= point.x && point.x <= self.x + self.w;
        }

        let in_y;
        if self.h < 0 {
            in_y = self.y + self.h <= point.y && point.y <= self.y;
        }
        else {
            in_y = self.y <= point.y && point.y <= self.y + self.h;
        }

        in_x && in_y
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
    // Distance in scene units from which anchor points (corners, edges) of the sprite can be dragged.
    const ANCHOR_RADIUS: f32 = 10.0;

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
            Positioning::Tile => Rect::scaled_from(self.rect, grid_size)
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

    fn grab_anchor(&mut self, at: ScenePoint, grid_size: i32) -> Option<HeldObject> {
        let (x, y, w, h) = self.absolute_rect(grid_size).as_floats();
        let at_x = at.x as f32;
        let at_y = at.y as f32;

        for dx in -1..2 {
            for dy in -1..2 {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let anchor_x = x + (w / 2.0) * (dx + 1) as f32;
                let anchor_y = y + (h / 2.0) * (dy + 1) as f32;

                let delta_x = anchor_x - at_x;
                let delta_y = anchor_y - at_y;

                if (delta_x.powi(2) + delta_y.powi(2)).sqrt() <= Sprite::ANCHOR_RADIUS {
                    return Some(
                        HeldObject::Anchor(self.id, dx, dy)
                    );
                }
            }
        }

        None
    }

    fn grab(&mut self, at: ScenePoint, grid_size: i32) -> HeldObject {
        self.tile_to_absolute(grid_size);
        self.grab_anchor(at, grid_size).unwrap_or_else(
            || HeldObject::Sprite(self.id, ScenePoint { x: at.x - self.x(grid_size), y: at.y - self.y(grid_size) })
        )
    }

    fn pos(&mut self) -> ScenePoint {
        ScenePoint { x: self.rect.x, y: self.rect.y }
    }

    fn anchor_point(&mut self, dx: i32, dy: i32, grid_size: i32) -> ScenePoint {
        let Rect {x, y, w, h} = self.absolute_rect(grid_size);
        ScenePoint { x: x + (w / 2) * (dx + 1), y: y + (h / 2) * (dy + 1) }
    }

    fn update_held_pos(&mut self, holding: HeldObject, at: ScenePoint, grid_size: i32) {
        match holding {
            HeldObject::Sprite(_, offset) => {
                self.set_pos(at - offset);
            },
            HeldObject::Anchor(_, dx, dy) => {
                let ScenePoint { x: delta_x, y: delta_y } = at - self.anchor_point(dx, dy, grid_size);
                let x = self.rect.x + (if dx == -1 { delta_x } else {0});
                let y = self.rect.y + (if dy == -1 { delta_y } else {0});
                let w = delta_x * dx + self.rect.w;
                let h = delta_y * dy + self.rect.h;

                self.rect = Rect { x, y, w, h }
            },
            _ => return // Other types aren't sprite-related
        };
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


#[derive(Clone, Copy)]
enum HeldObject {
    Map(ViewportPoint),
    None,
    Sprite(u32, ScenePoint),
    Anchor(u32, i32, i32)
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
                _ => 0
            }
        )
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
        let id = match self.holding {
            HeldObject::Map(ViewportPoint { x, y }) => {
                self.viewport_offset = ScenePoint {
                    x: self.viewport_offset.x + x - pos.x,
                    y: self.viewport_offset.y + y - pos.y
                };
                self.holding = HeldObject::Map(pos);
                self.redraw_needed = true;
                return;
            },
            HeldObject::None => return,
            HeldObject::Sprite(id, _) => id,
            HeldObject::Anchor(id, _, _) => id,
        };

        let holding = self.holding;
        let at = pos.apply_offset(self.viewport_offset);
        let grid_size = self.grid_size();
        self.sprite(id).map(|s| s.update_held_pos(holding, at, grid_size));

        self.redraw_needed = true;
    }

    fn release_held(&mut self) {
        let grid_size = self.grid_size();

        let held = {
            match self.holding {
                HeldObject::Map(_) => { self.holding = HeldObject::None; return; },
                HeldObject::Sprite(id, _) => id,
                HeldObject::Anchor(id, _, _) => id,
                HeldObject::None => return
            }
        };

        self.sprite(held).map(|s| s.absolute_to_tile(grid_size));
        self.holding = HeldObject::None;
        self.redraw_needed = true;
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
            let vp = self.viewport();
            let grid_size = self.grid_size();

            self.context.clear(vp);
            self.context.draw_grid(vp, grid_size);
            self.context.draw_sprites(vp, &self.sprites, grid_size);    


            let outline = self.held_sprite().map(|s| s.absolute_rect(grid_size));
            
            if let Some(rect) = outline {
                self.context.draw_outline(vp, rect);
            }
        }
        self.redraw_needed = false;
    }
}
