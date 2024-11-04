use serde::{Deserialize, Serialize};

use crate::Project;

type Res<T> = Result<T, String>;

#[derive(Serialize, Deserialize)]
struct Save {
    version: u32,
    data: Vec<u8>,
}

fn bincode_serialise(val: impl Serialize) -> Res<Vec<u8>> {
    bincode::serialize(&val).map_err(|e| e.to_string())
}

pub fn serialise(project: &Project) -> Res<Vec<u8>> {
    let data = bincode_serialise(&v1::prepare(project))?;
    bincode_serialise(Save { version: 1, data })
}

fn bincode_deserialise<'a, T: Deserialize<'a>>(data: &'a [u8]) -> Res<T> {
    bincode::deserialize(data).map_err(|e| e.to_string())
}

pub fn deserialise(data: &[u8]) -> Res<crate::Project> {
    let save: Save = bincode_deserialise(data)?;
    match save.version {
        1 => v1::retrieve(&save.data),
        v => {
            // Unknown serialisation version. Attempt to load as v1 in case
            // just the version is wrong.
            match v1::retrieve(&save.data) {
                Ok(proj) => Ok(proj),
                Err(_) => Err(format!("Unknown serialisation version: {v}")),
            }
        }
    }
}

mod v1 {
    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    use super::{bincode_deserialise, Res};
    use crate::{Id, PointVector};

    type IdMap = HashMap<Id, u32>;

    pub fn retrieve(data: &[u8]) -> Res<crate::Project> {
        let project: Project = bincode_deserialise(data)?;
        Ok(crate::Project {
            uuid: project.uuid,
            title: project.title,
            scenes: project
                .scenes
                .into_iter()
                .map(|scene| retrieve_scene(scene, project.uuid))
                .collect(),
        })
    }

    fn retrieve_scene(scene: Scene, project: Uuid) -> crate::Scene {
        let mut id = 1;

        let mut layer_idx_to_layer = HashMap::new();
        for (idx, layer) in scene.layers.into_iter().enumerate() {
            layer_idx_to_layer.insert(idx as u32, crate::Layer::new(id, &layer.title, layer.z));
            id += 1;
        }

        let mut drawing_idx_to_id = HashMap::new();
        let mut drawings = Vec::new();
        for (idx, drawing) in scene.drawings.into_iter().enumerate() {
            drawings.push(crate::Drawing::from(
                id,
                u8_to_mode(drawing.mode),
                PointVector::from(drawing.points),
            ));
            drawing_idx_to_id.insert(idx as u32, id);
            id += 1;
        }

        let mut sprite_idx_to_id = HashMap::new();
        for (idx, sprite) in scene.sprites.into_iter().enumerate() {
            if let (Some(visual), Some(layer)) = (
                retrieve_visual(&sprite, &drawing_idx_to_id),
                layer_idx_to_layer.get_mut(&sprite.layer),
            ) {
                layer.add_sprite(crate::Sprite {
                    id,
                    rect: crate::Rect::new(sprite.x, sprite.y, sprite.w, sprite.h),
                    z: sprite.z,
                    visual,
                });
                sprite_idx_to_id.insert(idx as u32, id);
                id += 1;
            }
        }

        let mut groups = Vec::new();
        for group in scene.groups {
            let mut group_sprites = Vec::new();
            for sprite in group.sprites {
                if let Some(sprite_id) = sprite_idx_to_id.get(&sprite) {
                    group_sprites.push(*sprite_id);
                }
            }
            groups.push(crate::Group::new(id, group_sprites));
            id += 1;
        }

        let layers = layer_idx_to_layer.into_values().collect();
        let mut sc = crate::Scene::new_with(project, layers, drawings);
        sc.uuid = Some(scene.uuid);
        sc.title = scene.title;
        sc.fog = crate::Fog::from(scene.fog, scene.fog_active, scene.w, scene.h);
        sc.groups = groups;
        sc
    }

    fn retrieve_visual(
        sprite: &Sprite,
        drawings: &HashMap<u32, Id>,
    ) -> Option<crate::SpriteVisual> {
        match &sprite.visual {
            SpriteVisual::Texture { shape, media } => Some(crate::SpriteVisual::Texture {
                shape: u8_to_shape(*shape),
                id: *media,
            }),
            SpriteVisual::Shape {
                shape,
                stroke,
                solid,
                colour,
            } => Some(crate::SpriteVisual::Shape {
                shape: u8_to_shape(*shape),
                stroke: *stroke,
                solid: *solid,
                colour: crate::Colour([colour.r, colour.g, colour.b, colour.a]),
            }),
            SpriteVisual::Drawing {
                drawing,
                colour,
                stroke,
                cap_start,
                cap_end,
            } => drawings
                .get(drawing)
                .map(|drawing| crate::SpriteVisual::Drawing {
                    drawing: *drawing,
                    colour: crate::Colour([colour.r, colour.g, colour.b, colour.a]),
                    stroke: *stroke,
                    cap_start: u8_to_cap(*cap_start),
                    cap_end: u8_to_cap(*cap_end),
                }),
        }
    }

    pub fn prepare(project: &crate::Project) -> Res<impl Serialize> {
        Ok(Project {
            uuid: project.uuid,
            title: project.title.clone(),
            scenes: project
                .scenes
                .iter()
                .map(prepare_scene)
                .collect::<Res<Vec<Scene>>>()?,
        })
    }

    fn prepare_scene(scene: &crate::Scene) -> Res<Scene> {
        let (drawings, drawing_ids_to_idxs) = prepare_drawings(scene);
        let (layers, sprites, sprite_ids_to_idxs) =
            prepare_layers_sprites(scene, &drawing_ids_to_idxs);
        let groups = prepare_groups(scene, &sprite_ids_to_idxs);
        let Some(uuid) = scene.uuid else {
            return Err("Can't save scene before it's assigned a UUID.".to_string());
        };
        Ok(Scene {
            uuid,
            title: scene.title.clone(),
            w: scene.fog.w,
            h: scene.fog.h,
            fog: scene.fog.data(),
            fog_active: scene.fog.active,
            layers,
            drawings,
            sprites,
            groups,
        })
    }

    fn prepare_drawings(scene: &crate::Scene) -> (Vec<Drawing>, IdMap) {
        let mut drawings = Vec::new();
        let mut id_to_idx = HashMap::new();
        for drawing in scene.get_drawings() {
            let idx = drawings.len();
            drawings.push(Drawing {
                mode: mode_to_u8(drawing.mode),
                points: drawing.points_build().data,
            });
            id_to_idx.insert(drawing.id, idx as u32);
        }
        (drawings, id_to_idx)
    }

    fn prepare_layers_sprites(
        scene: &crate::Scene,
        drawings: &IdMap,
    ) -> (Vec<Layer>, Vec<Sprite>, IdMap) {
        let mut layers = Vec::new();
        let mut sprites = Vec::new();
        let mut sprite_id_to_idx = HashMap::new();
        for layer in &scene.layers {
            let idx = layers.len() as u32;
            layers.push(Layer {
                title: layer.title.clone(),
                z: layer.z,
                visible: layer.visible,
                locked: layer.locked,
            });

            for sprite in &layer.sprites {
                if let Some(prepped) = prepare_sprite(sprite, idx, drawings) {
                    sprite_id_to_idx.insert(sprite.id, sprites.len() as u32);
                    sprites.push(prepped);
                }
            }
        }
        (layers, sprites, sprite_id_to_idx)
    }

    fn prepare_groups(scene: &crate::Scene, sprites: &IdMap) -> Vec<Group> {
        let mut groups = Vec::new();
        for group in &scene.groups {
            let mut group_idxs = Vec::new();
            for sprite in group.sprites() {
                if let Some(idx) = sprites.get(sprite) {
                    group_idxs.push(*idx);
                }
            }
            if !group_idxs.is_empty() {
                groups.push(Group {
                    sprites: group_idxs,
                });
            }
        }
        groups
    }

    fn prepare_sprite(sprite: &crate::Sprite, layer: u32, drawings: &IdMap) -> Option<Sprite> {
        let visual = match sprite.visual {
            crate::SpriteVisual::Texture { shape, id } => SpriteVisual::Texture {
                shape: shape_to_u8(shape),
                media: id,
            },
            crate::SpriteVisual::Shape {
                shape,
                stroke,
                solid,
                colour,
            } => SpriteVisual::Shape {
                shape: shape_to_u8(shape),
                stroke,
                solid,
                colour: prepare_colour(&colour),
            },
            crate::SpriteVisual::Drawing {
                drawing,
                colour,
                stroke,
                cap_start,
                cap_end,
            } => SpriteVisual::Drawing {
                drawing: drawings.get(&drawing).copied()?,
                colour: prepare_colour(&colour),
                stroke,
                cap_start: cap_to_u8(cap_start),
                cap_end: cap_to_u8(cap_end),
            },
        };

        Some(Sprite {
            layer,
            x: sprite.rect.x,
            y: sprite.rect.y,
            w: sprite.rect.w,
            h: sprite.rect.h,
            z: sprite.z,
            visual,
        })
    }

    fn prepare_colour(colour: &crate::Colour) -> Colour {
        Colour {
            r: colour.r(),
            g: colour.g(),
            b: colour.b(),
            a: colour.a(),
        }
    }

    fn mode_to_u8(mode: crate::DrawingMode) -> u8 {
        match mode {
            crate::DrawingMode::Cone => 1,
            crate::DrawingMode::Freehand => 2,
            crate::DrawingMode::Line => 3,
        }
    }

    fn u8_to_mode(int: u8) -> crate::DrawingMode {
        match int {
            1 => crate::DrawingMode::Cone,
            2 => crate::DrawingMode::Freehand,
            3 => crate::DrawingMode::Line,
            _ => crate::DrawingMode::Freehand,
        }
    }

    fn shape_to_u8(shape: crate::Shape) -> u8 {
        match shape {
            crate::Shape::Ellipse => 1,
            crate::Shape::Hexagon => 2,
            crate::Shape::Triangle => 3,
            crate::Shape::Rectangle => 4,
        }
    }

    fn u8_to_shape(int: u8) -> crate::Shape {
        match int {
            1 => crate::Shape::Ellipse,
            2 => crate::Shape::Hexagon,
            3 => crate::Shape::Triangle,
            _ => crate::Shape::Rectangle,
        }
    }

    fn cap_to_u8(cap: crate::Cap) -> u8 {
        match cap {
            crate::Cap::Arrow => 1,
            crate::Cap::Round => 2,
            crate::Cap::None => u8::MAX,
        }
    }

    fn u8_to_cap(int: u8) -> crate::Cap {
        match int {
            1 => crate::Cap::Arrow,
            2 => crate::Cap::Round,
            _ => crate::Cap::None,
        }
    }

    #[derive(Serialize, Deserialize)]
    struct Project {
        uuid: Uuid,
        title: String,
        scenes: Vec<Scene>,
    }

    #[derive(Serialize, Deserialize)]
    struct Scene {
        uuid: Uuid,
        title: String,
        w: u32,
        h: u32,
        fog: Vec<u32>,
        fog_active: bool,
        layers: Vec<Layer>,
        drawings: Vec<Drawing>,
        sprites: Vec<Sprite>,
        groups: Vec<Group>,
    }

    #[derive(Serialize, Deserialize)]
    struct Layer {
        title: String,
        z: i32,
        visible: bool,
        locked: bool,
    }

    #[derive(Serialize, Deserialize)]
    struct Group {
        sprites: Vec<u32>, // Indices into sprites vector.
    }

    #[derive(Serialize, Deserialize)]
    struct Sprite {
        layer: u32, // Index into layers vector.
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        z: i32,
        visual: SpriteVisual,
    }

    #[derive(Serialize, Deserialize)]
    struct Colour {
        r: f32,
        g: f32,
        b: f32,
        a: f32,
    }

    #[derive(Serialize, Deserialize)]
    enum SpriteVisual {
        Texture {
            shape: u8,
            media: Id,
        },
        Shape {
            shape: u8,
            stroke: f32,
            solid: bool,
            colour: Colour,
        },
        Drawing {
            drawing: u32,
            colour: Colour,
            stroke: f32,
            cap_start: u8,
            cap_end: u8,
        },
    }

    #[derive(Serialize, Deserialize)]
    struct Drawing {
        mode: u8,
        points: Vec<f32>,
    }
}
