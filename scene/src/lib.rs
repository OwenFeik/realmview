#![allow(dead_code)]

use std::ops::{Add, Sub};
use std::sync::atomic::{AtomicI64, Ordering};

use serde_derive::{Serialize, Deserialize};

pub type Id = i64;

#[derive(Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    pub fn from_point(point: ScenePoint, w: f32, h: f32) -> Rect {
        Rect {
            x: point.x,
            y: point.y,
            w,
            h,
        }
    }

    pub fn scaled_from(from: Rect, factor: f32) -> Rect {
        let mut rect = from;
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

        if self.w > 0.0 && self.w < 1.0 {
            self.w = 1.0;
        } else if self.w < 0.0 && self.w > -1.0 {
            self.w = -1.0;
        }

        if self.h > 0.0 && self.h < 1.0 {
            self.h = 1.0;
        } else if self.h < 0.0 && self.h > -1.0 {
            self.h = -1.0;
        }
    }

    fn contains_point(&self, point: ScenePoint) -> bool {
        // A negative dimension causes a texture to be flipped. As this is a useful behaviour, negative dimensions on
        // Rects are supported. To that end a different treatment is required for checking if a point is contained.
        // Hence the special cases for negative width and height.

        let in_x = {
            if self.w < 0.0 {
                self.x + self.w <= point.x && point.x <= self.x
            } else {
                self.x <= point.x && point.x <= self.x + self.w
            }
        };

        let in_y = {
            if self.h < 0.0 {
                self.y + self.h <= point.y && point.y <= self.y
            } else {
                self.y <= point.y && point.y <= self.y + self.h
            }
        };

        in_x && in_y
    }

    pub fn top_left(&self) -> ScenePoint {
        ScenePoint {
            x: self.x,
            y: self.y,
        }
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct Sprite {
    pub rect: Rect,

    pub z: i32,

    // id pointing to the texture associated with this Sprite
    pub texture: Id,

    // Unique numeric ID, numbered from 1
    id: Id,
}

impl Sprite {
    // Distance in scene units from which anchor points (corners, edges) of the
    // sprite can be dragged.
    const ANCHOR_RADIUS: f32 = 0.2;

    pub fn new(texture: Id) -> Sprite {
        static SPRITE_ID: AtomicI64 = AtomicI64::new(1);

        Sprite {
            rect: Rect::new(0.0, 0.0, 1.0, 1.0),
            z: 1,
            texture,
            id: SPRITE_ID.fetch_add(1, Ordering::Relaxed),
        }
    }

    fn set_pos(&mut self, ScenePoint { x, y }: ScenePoint) {
        self.rect.x = x;
        self.rect.y = y;
    }

    fn set_size(&mut self, w: f32, h: f32) {
        self.rect.w = w;
        self.rect.h = h;
    }

    fn set_texture(&mut self, new: Id) {
        self.texture = new;
    }

    fn snap_to_grid(&mut self) {
        self.rect.round();
    }

    fn grab_anchor(&mut self, at: ScenePoint) -> Option<HeldObject> {
        let Rect { x, y, w, h } = self.rect;

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
        self.grab_anchor(at).unwrap_or({
            HeldObject::Sprite(
                self.id,
                ScenePoint {
                    x: at.x - self.rect.x,
                    y: at.y - self.rect.y,
                },
            )
        })
    }

    pub fn pos(&self) -> ScenePoint {
        ScenePoint {
            x: self.rect.x,
            y: self.rect.y,
        }
    }

    fn anchor_point(&mut self, dx: i32, dy: i32) -> ScenePoint {
        let Rect { x, y, w, h } = self.rect;
        ScenePoint {
            x: x + (w / 2.0) * (dx + 1) as f32,
            y: y + (h / 2.0) * (dy + 1) as f32,
        }
    }

    fn update_held_pos(&mut self, holding: HeldObject, at: ScenePoint) {
        match holding {
            HeldObject::Sprite(_, offset) => {
                self.set_pos(at - offset);
            }
            HeldObject::Anchor(_, dx, dy) => {
                let ScenePoint {
                    x: delta_x,
                    y: delta_y,
                } = at - self.anchor_point(dx, dy);
                let x = self.rect.x + (if dx == -1 { delta_x } else { 0.0 });
                let y = self.rect.y + (if dy == -1 { delta_y } else { 0.0 });
                let w = delta_x * (dx as f32) + self.rect.w;
                let h = delta_y * (dy as f32) + self.rect.h;

                self.rect = Rect { x, y, w, h }
            }
            _ => (), // Other types aren't sprite-related
        }
    }
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq)]
pub struct ScenePoint {
    pub x: f32,
    pub y: f32,
}

impl ScenePoint {
    pub fn new(x: f32, y: f32) -> ScenePoint {
        ScenePoint { x, y }
    }
}

impl Add for ScenePoint {
    type Output = ScenePoint;

    fn add(self, rhs: ScenePoint) -> ScenePoint {
        ScenePoint {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for ScenePoint {
    type Output = ScenePoint;

    fn sub(self, rhs: ScenePoint) -> ScenePoint {
        ScenePoint {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

#[derive(Clone, Copy)]
enum HeldObject {
    None,
    Sprite(Id, ScenePoint),
    Anchor(Id, i32, i32),
}


struct SpriteSet {
    sprites: Vec<Sprite>,
    z_min: i32,
    z_max: i32
}

impl SpriteSet {
    fn new() -> Self {
        SpriteSet {
            sprites: Vec::new(),
            z_min: 0,
            z_max: 0
        }
    }

    fn sprite(&mut self, id: Id) -> Option<&mut Sprite> {
        self.sprites.iter_mut().find(|s| s.id == id)
    }

    fn sort_sprites(&mut self) {
        self.sprites.sort_by(|a, b| a.z.cmp(&b.z));
    }

    fn update_z_bounds(&mut self, sprite: &Sprite) {
        if sprite.z > self.z_max {
            self.z_max = sprite.z;
        }
        else if sprite.z < self.z_min {
            self.z_min = sprite.z;
        }    
    }

    fn add_sprite(&mut self, sprite: Sprite) {
        self.update_z_bounds(&sprite);
        self.sprites.push(sprite);
        self.sort_sprites();
    }

    fn add_sprites(&mut self, sprites: &mut Vec<Sprite>) {
        for s in sprites.iter() {
            self.update_z_bounds(s);
        }
        self.sprites.append(sprites);
        self.sort_sprites();
    }

    fn sprite_at(&mut self, at: ScenePoint) -> Option<&mut Sprite> {
        // Reversing the iterator atm because the sprites are rendered from the
        // front of the Vec to the back, hence the last Sprite in the Vec is
        // rendered on top, and will be clicked first.
        for sprite in self.sprites.iter_mut().rev() {
            if sprite.rect.contains_point(at) {
                return Some(sprite);
            }
        }

        None
    }
}


#[derive(Deserialize, Serialize)]
pub enum SceneEvent {
    SpriteNew(Sprite, bool), // (new_sprite, is_token) 
    SpriteMove(Id, ScenePoint, ScenePoint), // (sprite_id, from, to)
    SpriteTextureChange(Id, Id, Id), // (sprite_id, old_texture, new_texture)
}

pub struct Scene {
    scenery: SpriteSet,
    tokens: SpriteSet,

    // ID of the Sprite the user is currently dragging.
    holding: HeldObject,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            scenery: SpriteSet::new(),
            tokens: SpriteSet::new(),
            holding: HeldObject::None,
        }
    }

    fn sprite(&mut self, id: Id) -> Option<&mut Sprite> {
        self.tokens.sprite(id).or_else(|| self.scenery.sprite(id))
    } 

    fn sprite_at(&mut self, at: ScenePoint, include_scenery: bool) -> Option<&mut Sprite> {
        self.tokens.sprite_at(at).or_else(|| {
            if include_scenery {
                self.scenery.sprite_at(at)
            }
            else {
                None
            }
        })
    }

    pub fn scenery(&self) -> &Vec<Sprite> {
        &self.scenery.sprites
    }

    pub fn add_scenery(&mut self, sprites: &mut Vec<Sprite>) {
        self.scenery.add_sprites(sprites);
    }

    pub fn tokens(&self) -> &Vec<Sprite> {
        &self.tokens.sprites
    }

    pub fn add_tokens(&mut self, sprites: &mut Vec<Sprite>) {
        self.tokens.add_sprites(sprites);
    }

    pub fn add_sprite(&mut self, sprite: Sprite, is_scenery: bool) {
        if is_scenery {
            self.scenery.add_sprite(sprite);
        }
        else {
            self.tokens.add_sprite(sprite);
        }
    }

    pub fn held_id(&self) -> Option<Id> {
        match self.holding {
            HeldObject::Sprite(id, _) => Some(id),
            HeldObject::Anchor(id, _, _) => Some(id),
            _ => None,
        }
    }

    pub fn held_sprite(&mut self) -> Option<&mut Sprite> {
        match self.held_id() {
            Some(id) => self.sprite(id),
            None => None 
        }
    }

    pub fn update_held_pos(&mut self, at: ScenePoint) -> bool {
        let holding = self.holding;
        self.held_sprite()
            .map(|s| s.update_held_pos(holding, at))
            .is_some()
    }

    pub fn release_held(&mut self, snap_to_grid: bool) -> bool {
        let redraw_needed = {
            if snap_to_grid {
                self.held_sprite().map(|s| s.snap_to_grid()).is_some()
            } else {
                false
            }
        };

        self.holding = HeldObject::None;
        redraw_needed
    }

    pub fn grab(&mut self, at: ScenePoint, include_scenery: bool) -> bool {
        match self.sprite_at(at, include_scenery) {
            Some(s) => {
                self.holding = s.grab(at);
                true
            }
            None => false,
        }
    }

    pub fn apply_event(&mut self, event: SceneEvent, canonical: bool) -> bool {
        match event {
            SceneEvent::SpriteNew(s, is_scenery) => {
                if self.sprite(s.id).is_none() {
                    self.add_sprite(s, is_scenery);
                    true
                }
                else {
                    false
                }
            },
            SceneEvent::SpriteMove(id, from, to) => {
                self.held_id().map(|held| {
                    if held == id {
                        self.release_held(false);
                    }
                });

                match self.sprite(id) {
                    Some(s) if s.pos() == from || canonical => {
                            s.set_pos(to);
                            true
                    }
                    _ => false
                }
            },
            SceneEvent::SpriteTextureChange(id, old, new) => {
                match self.sprite(id) {
                    Some(s) if s.texture == old || canonical => {
                        s.set_texture(new);
                        true
                    },
                    _ => false
                }
            }
        }
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}
