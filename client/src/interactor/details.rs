use scene::{
    comms::SceneEvent, Colour, Dimension, Id, Scene, Sprite, SpriteDrawing, SpriteShape,
    SpriteVisual,
};

#[derive(Debug, Default, serde_derive::Deserialize, serde_derive::Serialize)]
#[serde(default)]
pub struct SceneDetails {
    pub id: Option<Id>,
    pub title: Option<String>,
    pub w: Option<u32>,
    pub h: Option<u32>,
}

impl SceneDetails {
    pub fn from(scene: &Scene) -> Self {
        SceneDetails {
            id: scene.id,
            title: scene.title.clone(),
            w: Some(scene.w),
            h: Some(scene.h),
        }
    }

    pub fn update_scene(&self, scene: &mut Scene) {
        if self.title.is_some() {
            scene.title = self.title.clone();
        }

        if let Some(w) = self.w {
            scene.w = w;
        }

        if let Some(h) = self.h {
            scene.h = h;
        }
    }
}

#[derive(Debug, Default, serde_derive::Deserialize, serde_derive::Serialize)]
#[serde(default)]
pub struct SpriteDetails {
    pub id: Id,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub w: Option<f32>,
    pub h: Option<f32>,
    pub shape: Option<SpriteShape>,
    pub stroke: Option<f32>,
    pub colour: Option<Colour>,
    pub texture: Option<Id>,
    pub drawing_type: Option<scene::SpriteDrawingType>,
    pub cap_start: Option<scene::SpriteCap>,
    pub cap_end: Option<scene::SpriteCap>,
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
            colour: sprite.visual.colour(),
            texture: sprite.visual.texture(),
            drawing_type: sprite.visual.drawing().map(|d| d.drawing_type),
            cap_start: sprite.visual.cap_start(),
            cap_end: sprite.visual.cap_end(),
        }
    }

    pub fn drawing(&self) -> SpriteVisual {
        SpriteVisual::Drawing(SpriteDrawing {
            drawing_type: self.drawing_type(),
            colour: self.colour(),
            cap_start: self.cap_start(),
            cap_end: self.cap_end(),
            stroke: self.stroke(),
            ..Default::default()
        })
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

        if other.colour.is_some() {
            self.colour = other.colour;
        }

        if other.texture.is_some() {
            self.texture = other.texture;
        }

        if other.drawing_type.is_some() {
            self.drawing_type = other.drawing_type;
        }

        if other.cap_start.is_some() {
            self.cap_start = other.cap_start;
        }

        if other.cap_end.is_some() {
            self.cap_end = other.cap_end;
        }
    }

    pub fn colour(&self) -> Colour {
        self.colour.unwrap_or(Sprite::DEFAULT_COLOUR)
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

        if self.colour.is_some() && self.colour != sprite.visual.colour() {
            self.colour = None;
        }

        if self.texture.is_some() && self.texture != sprite.visual.texture() {
            self.texture = None;
        }

        if self.drawing_type.is_some()
            && self.drawing_type != sprite.visual.drawing().map(|d| d.drawing_type)
        {
            self.drawing_type = None;
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

        if let Some(drawing_type) = self.drawing_type {
            if let Some(event) = sprite.set_drawing_type(drawing_type) {
                events.push(event);
            }
        }

        if let Some(event) = sprite.set_caps(self.cap_start, self.cap_end) {
            events.push(event);
        }

        if events.is_empty() {
            None
        } else {
            Some(SceneEvent::EventSet(events))
        }
    }

    fn stroke(&self) -> f32 {
        self.stroke.unwrap_or(Sprite::DEFAULT_STROKE)
    }

    fn drawing_type(&self) -> scene::SpriteDrawingType {
        self.drawing_type
            .unwrap_or(scene::SpriteDrawing::DEFAULT_TYPE)
    }

    fn cap_start(&self) -> scene::SpriteCap {
        self.cap_start.unwrap_or(scene::SpriteCap::DEFAULT_START)
    }

    fn cap_end(&self) -> scene::SpriteCap {
        self.cap_end.unwrap_or(scene::SpriteCap::DEFAULT_END)
    }
}