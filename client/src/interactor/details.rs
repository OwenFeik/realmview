use scene::{comms::SceneEvent, Colour, Dimension, Id, Scene, Shape, Sprite, SpriteVisual};
use uuid::Uuid;

#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct SceneDetails {
    pub uuid: Option<Uuid>,
    pub title: Option<String>,
    pub w: Option<u32>,
    pub h: Option<u32>,
    pub fog: Option<bool>,
}

impl SceneDetails {
    pub fn from(scene: &Scene) -> Self {
        SceneDetails {
            uuid: Some(scene.uuid),
            title: Some(scene.title.clone()),
            w: Some(scene.w()),
            h: Some(scene.h()),
            fog: Some(scene.fog.active),
        }
    }

    pub fn update_scene(&self, scene: &mut Scene) -> Option<SceneEvent> {
        let mut events = Vec::new();
        if let Some(title) = &self.title
            && title != &scene.title
        {
            let old = scene.title.clone();
            scene.title.clone_from(title);
            events.push(SceneEvent::SceneTitle(old, self.title.clone().unwrap()));
        }

        match (self.w, self.h) {
            (Some(w), Some(h)) => events.push(scene.set_size(w, h)),
            (Some(w), None) => events.push(scene.set_size(w, scene.h())),
            (None, Some(h)) => events.push(scene.set_size(scene.w(), h)),
            _ => {}
        }

        if self.fog.is_some() {
            if let Some(event) = scene.fog.set_active(self.fog.unwrap()) {
                events.push(event);
            }
        }

        SceneEvent::set(events)
    }
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct SpriteDetails {
    pub id: Id,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub w: Option<f32>,
    pub h: Option<f32>,
    pub shape: Option<Shape>,
    pub stroke: Option<f32>,
    pub solid: Option<bool>,
    pub colour: Option<Colour>,
    pub texture: Option<Id>,
    pub cap_start: Option<scene::Cap>,
    pub cap_end: Option<scene::Cap>,
}

impl SpriteDetails {
    pub fn from(id: Id, sprite: &Sprite) -> Self {
        SpriteDetails {
            id,
            x: Some(sprite.rect.x),
            y: Some(sprite.rect.y),
            w: Some(sprite.rect.w),
            h: Some(sprite.rect.h),
            shape: sprite.visual.shape(),
            stroke: sprite.visual.stroke(),
            solid: sprite.visual.solid(),
            colour: sprite.visual.colour(),
            texture: sprite.visual.texture(),
            cap_start: sprite.visual.cap_start(),
            cap_end: sprite.visual.cap_end(),
        }
    }

    pub fn drawing(&self) -> SpriteVisual {
        let stroke = if self.stroke() < f32::EPSILON {
            Sprite::DEFAULT_STROKE
        } else {
            self.stroke()
        };

        SpriteVisual::Drawing {
            drawing: 0,
            colour: self.colour(),
            cap_start: self.cap_start(),
            cap_end: self.cap_end(),
            stroke,
        }
    }

    pub fn update_from(&mut self, other: &Self) {
        self.id = other.id;

        if other.x.is_some() {
            self.x = other.x;
        }

        if other.y.is_some() {
            self.y = other.y;
        }

        if other.w.is_some() {
            self.w = other.w;
        }

        if other.h.is_some() {
            self.h = other.h;
        }

        // Special case for shape because setting to no shape is meaningful
        self.shape = other.shape;

        if other.stroke.is_some() {
            self.stroke = other.stroke;
        }

        if other.solid.is_some() {
            self.solid = other.solid;
        }

        if other.colour.is_some() {
            self.colour = other.colour;
        }

        if other.texture.is_some() {
            self.texture = other.texture;
        }

        if other.cap_start.is_some() {
            self.cap_start = other.cap_start;
        }

        if other.cap_end.is_some() {
            self.cap_end = other.cap_end;
        }
    }

    pub fn colour(&self) -> Colour {
        self.colour.unwrap_or(Colour::DEFAULT)
    }

    pub fn common(&mut self, sprite: &Sprite) {
        if self.x != Some(sprite.rect.x) {
            self.x = None;
        }

        if self.y != Some(sprite.rect.y) {
            self.y = None;
        }

        if self.w != Some(sprite.rect.w) {
            self.w = None;
        }

        if self.h != Some(sprite.rect.h) {
            self.h = None;
        }

        if self.shape.is_some() && self.shape != sprite.visual.shape() {
            self.shape = None;
        }

        if self.stroke.is_some() && self.stroke != sprite.visual.stroke() {
            self.stroke = None;
        }

        if self.solid.is_some() && self.solid != sprite.visual.solid() {
            self.solid = None;
        }

        if self.colour.is_some() && self.colour != sprite.visual.colour() {
            self.colour = None;
        }

        if self.texture.is_some() && self.texture != sprite.visual.texture() {
            self.texture = None;
        }

        if self.cap_start.is_some() && self.cap_start != sprite.visual.cap_start() {
            self.cap_start = None;
        }

        if self.cap_end.is_some() && self.cap_end != sprite.visual.cap_end() {
            self.cap_end = None;
        }
    }

    pub fn update_sprite(&self, sprite: &mut Sprite) -> Option<SceneEvent> {
        let mut events = vec![];
        if let Some(x) = self.x {
            events.push(sprite.set_dimension(Dimension::X, x));
        }

        if let Some(y) = self.y {
            events.push(sprite.set_dimension(Dimension::Y, y));
        }

        if let Some(w) = self.w {
            events.push(sprite.set_dimension(Dimension::W, w));
        }

        if let Some(h) = self.h {
            events.push(sprite.set_dimension(Dimension::H, h));
        }

        if let Some(shape) = self.shape {
            if let Some(event) = sprite.set_shape(shape) {
                events.push(event);
            }
        }

        if let Some(stroke) = self.stroke {
            if let Some(event) = sprite.set_stroke(stroke) {
                events.push(event);
            }
        }

        if let Some(solid) = self.solid {
            if let Some(event) = sprite.set_solid(solid) {
                events.push(event);
            }
        }

        if let Some(c) = self.colour {
            if let Some(event) = sprite.set_colour(c) {
                events.push(event);
            }
        }

        if let Some(id) = self.texture {
            if let Some(event) = sprite.set_texture(id) {
                events.push(event);
            }
        }

        if let Some(event) = sprite.set_caps(self.cap_start, self.cap_end) {
            events.push(event);
        }

        SceneEvent::set(events)
    }

    pub fn stroke(&self) -> f32 {
        self.stroke.unwrap_or(Sprite::DEFAULT_STROKE)
    }

    pub fn solid(&self) -> bool {
        self.solid.unwrap_or(false)
    }

    fn cap_start(&self) -> scene::Cap {
        self.cap_start.unwrap_or(scene::Cap::DEFAULT_START)
    }

    fn cap_end(&self) -> scene::Cap {
        self.cap_end.unwrap_or(scene::Cap::DEFAULT_END)
    }
}
