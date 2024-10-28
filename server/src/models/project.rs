use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use uuid::Uuid;

use crate::utils::Res;

type Conn = SqliteConnection;

#[derive(Serialize, Deserialize)]
struct Save {
    version: u32,
    data: Vec<u8>,
}

#[derive(sqlx::FromRow)]
pub struct SceneRecord {
    uuid: Uuid,
    project: i64,
    updated_time: i64,
    title: Option<String>,
    thumbnail: Option<String>,
}

impl SceneRecord {
    pub async fn load_by_uuid(conn: &mut Conn, uuid: Uuid) -> Res<SceneRecord> {
        sqlx::query_as(
            "
            SELECT (uuid, project, updated_time, title, thumbnail)
            FROM scenes WHERE uuid = ?1; 
            ",
        )
        .bind(uuid.simple().to_string())
        .fetch_one(conn)
        .await
        .map_err(|e| e.to_string())
    }
}

#[derive(sqlx::FromRow)]
pub struct ProjectRecord {
    pub uuid: Uuid,
    pub user: i64,
    pub updated_time: i64,
    pub title: Option<String>,
}

impl ProjectRecord {
    pub async fn get_by_uuid(conn: &mut Conn, uuid: Uuid) -> Res<ProjectRecord> {
        sqlx::query_as(
            "
            SELECT (uuid, user, updated_time, title)
            FROM projects WHERE uuid = ?1;
            ",
        )
        .bind(uuid.simple().to_string())
        .fetch_one(conn)
        .await
        .map_err(|e| e.to_string())
    }
}

fn bincode_serialise(val: impl Serialize) -> Res<Vec<u8>> {
    bincode::serialize(&val).map_err(|e| e.to_string())
}

pub fn serialise(project: &scene::Project) -> Res<Vec<u8>> {
    let data = bincode_serialise(&v1::prepare(project))?;
    bincode_serialise(Save { version: 1, data })
}

fn bincode_deserialise<'a, T: Deserialize<'a>>(data: &'a [u8]) -> Res<T> {
    bincode::deserialize(data).map_err(|e| e.to_string())
}

pub fn deserialise(data: &[u8]) -> Res<scene::Project> {
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

mod db {
    use sqlx::SqliteConnection;
    use uuid::Uuid;

    use super::{ProjectRecord, SceneRecord};
    use crate::utils::{timestamp_s, Res};

    async fn update_database(
        conn: &mut SqliteConnection,
        project: &scene::Project,
        user: i64,
    ) -> Res<()> {
        update_or_create_project_record(conn, project, user).await?;
        for scene in &project.scenes {
            update_or_create_scene_record(conn, project.id, scene).await?;
        }
        Ok(())
    }

    async fn remove_deleted_scenes(conn: &mut SqliteConnection, project: &scene::Project) {
        let scene_ids = project
            .scenes
            .iter()
            .map(|scene| scene.id.to_string())
            .collect::<Vec<String>>()
            .join(", ");
        sqlx::query(&format!(
            "DELETE FROM scenes WHERE project = ?1 AND id NOT IN ({})",
            scene_ids
        ))
        .bind(project.id)
        .execute(conn)
        .await
        .ok();
    }

    async fn update_project_record(
        conn: &mut SqliteConnection,
        project_id: i64,
    ) -> Res<Option<ProjectRecord>> {
        match sqlx::query_as(
            "UPDATE projects SET updated_time = ?1, title = ?2 WHERE id = ?3 RETURNING *;",
        )
        .bind(project_id)
        .fetch_one(conn)
        .await
        {
            Ok(record) => Ok(Some(record)),
            Err(sqlx::Error::RowNotFound) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    async fn update_or_create_project_record(
        conn: &mut SqliteConnection,
        project: &scene::Project,
        user: i64,
    ) -> Res<ProjectRecord> {
        let record = update_project_record(conn, project.id).await?;
        let now = timestamp_s().unwrap_or(0) as i64;
        if let Some(record) = record {
            Ok(record)
        } else {
            sqlx::query_as(
                r#"
                INSERT INTO projects (project_key, user, updated_time, title)
                VALUES (?1, ?2, ?3, ?4) RETURNING *;
                "#,
            )
            .bind(&project.key)
            .bind(user)
            .bind(now)
            .bind(&project.title)
            .fetch_one(conn)
            .await
            .map_err(|e| e.to_string())
        }
    }

    async fn update_scene_record(
        conn: &mut SqliteConnection,
        scene: &scene::Scene,
        project: Uuid,
    ) -> Res<Option<ProjectRecord>> {
        match sqlx::query_as(
            "UPDATE scenes SET updated_time = ?1, title = ?2 WHERE id = ?3 RETURNING *;",
        )
        .bind(updated_timestamp())
        .bind(&scene.title)
        .bind(project.simple().to_string())
        .fetch_one(conn)
        .await
        {
            Ok(record) => Ok(Some(record)),
            Err(sqlx::Error::RowNotFound) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    async fn update_or_create_scene_record(
        conn: &mut SqliteConnection,
        project_id: i64,
        scene: &scene::Scene,
    ) -> Res<SceneRecord> {
        match sqlx::query_as(
            "UPDATE scenes SET updated_time = ?1, title = ?2 WHERE uuid = ?3 RETURNING *;",
        )
        .bind(updated_timestamp())
        .bind(scene.title)
        .bind(scene.id)
        .fetch_one(conn)
        .await
        {
            Ok(record) => Ok(Some(record)),
            Err(sqlx::Error::RowNotFound) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn updated_timestamp() -> i64 {
        timestamp_s().unwrap_or(0) as i64
    }
}

mod v1 {
    use std::collections::HashMap;

    use scene::{Id, PointVector};
    use serde::{Deserialize, Serialize};

    use super::bincode_deserialise;
    use crate::utils::{id_to_key, Res};

    type IdMap = HashMap<Id, u32>;

    pub fn retrieve(data: &[u8]) -> Res<scene::Project> {
        let project: Project = bincode_deserialise(data)?;
        Ok(scene::Project {
            id: project.id,
            key: id_to_key(project.id),
            title: project.title,
            scenes: project
                .scenes
                .into_iter()
                .map(|scene| retrieve_scene(scene, project.id))
                .collect(),
        })
    }

    fn retrieve_scene(scene: Scene, project_id: Id) -> scene::Scene {
        let mut id = 1;

        let mut layer_idx_to_layer = HashMap::new();
        for (idx, layer) in scene.layers.into_iter().enumerate() {
            layer_idx_to_layer.insert(idx as u32, scene::Layer::new(id, &layer.title, layer.z));
            id += 1;
        }

        let mut drawing_idx_to_id = HashMap::new();
        let mut drawings = Vec::new();
        for (idx, drawing) in scene.drawings.into_iter().enumerate() {
            drawings.push(scene::Drawing::from(
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
                layer.add_sprite(scene::Sprite {
                    id,
                    rect: scene::Rect::new(sprite.x, sprite.y, sprite.w, sprite.h),
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
            groups.push(scene::Group::new(id, group_sprites));
            id += 1;
        }

        let layers = layer_idx_to_layer.into_values().collect();
        let mut sc = scene::Scene::new_with(layers, drawings);
        sc.id = scene.id;
        sc.title = Some(scene.title);
        sc.project = Some(project_id);
        sc.fog = scene::Fog::from(scene.fog, scene.fog_active, scene.w, scene.h);
        sc.groups = groups;
        sc
    }

    fn retrieve_visual(
        sprite: &Sprite,
        drawings: &HashMap<u32, Id>,
    ) -> Option<scene::SpriteVisual> {
        match &sprite.visual {
            SpriteVisual::Texture { shape, media } => Some(scene::SpriteVisual::Texture {
                shape: u8_to_shape(*shape),
                id: *media,
            }),
            SpriteVisual::Shape {
                shape,
                stroke,
                solid,
                colour,
            } => Some(scene::SpriteVisual::Shape {
                shape: u8_to_shape(*shape),
                stroke: *stroke,
                solid: *solid,
                colour: scene::Colour([colour.r, colour.g, colour.b, colour.a]),
            }),
            SpriteVisual::Drawing {
                drawing,
                colour,
                stroke,
                cap_start,
                cap_end,
            } => drawings
                .get(drawing)
                .map(|drawing| scene::SpriteVisual::Drawing {
                    drawing: *drawing,
                    colour: scene::Colour([colour.r, colour.g, colour.b, colour.a]),
                    stroke: *stroke,
                    cap_start: u8_to_cap(*cap_start),
                    cap_end: u8_to_cap(*cap_end),
                }),
        }
    }

    pub fn prepare(project: &scene::Project) -> Res<impl Serialize> {
        Ok(Project {
            id: project.id,
            title: project.title.clone(),
            scenes: project
                .scenes
                .iter()
                .map(prepare_scene)
                .collect::<Res<Vec<Scene>>>()?,
        })
    }

    fn prepare_scene(scene: &scene::Scene) -> Res<Scene> {
        let (drawings, drawing_ids_to_idxs) = prepare_drawings(scene);
        let (layers, sprites, sprite_ids_to_idxs) =
            prepare_layers_sprites(scene, &drawing_ids_to_idxs);
        let groups = prepare_groups(scene, &sprite_ids_to_idxs);
        Ok(Scene {
            id: scene.id,
            title: scene.title.clone().unwrap_or("Untitled".into()),
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

    fn prepare_drawings(scene: &scene::Scene) -> (Vec<Drawing>, IdMap) {
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
        scene: &scene::Scene,
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

    fn prepare_groups(scene: &scene::Scene, sprites: &IdMap) -> Vec<Group> {
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

    fn prepare_sprite(sprite: &scene::Sprite, layer: u32, drawings: &IdMap) -> Option<Sprite> {
        let visual = match sprite.visual {
            scene::SpriteVisual::Texture { shape, id } => SpriteVisual::Texture {
                shape: shape_to_u8(shape),
                media: id,
            },
            scene::SpriteVisual::Shape {
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
            scene::SpriteVisual::Drawing {
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

    fn prepare_colour(colour: &scene::Colour) -> Colour {
        Colour {
            r: colour.r(),
            g: colour.g(),
            b: colour.b(),
            a: colour.a(),
        }
    }

    fn mode_to_u8(mode: scene::DrawingMode) -> u8 {
        match mode {
            scene::DrawingMode::Cone => 1,
            scene::DrawingMode::Freehand => 2,
            scene::DrawingMode::Line => 3,
        }
    }

    fn u8_to_mode(int: u8) -> scene::DrawingMode {
        match int {
            1 => scene::DrawingMode::Cone,
            2 => scene::DrawingMode::Freehand,
            3 => scene::DrawingMode::Line,
            _ => scene::DrawingMode::Freehand,
        }
    }

    fn shape_to_u8(shape: scene::Shape) -> u8 {
        match shape {
            scene::Shape::Ellipse => 1,
            scene::Shape::Hexagon => 2,
            scene::Shape::Triangle => 3,
            scene::Shape::Rectangle => 4,
        }
    }

    fn u8_to_shape(int: u8) -> scene::Shape {
        match int {
            1 => scene::Shape::Ellipse,
            2 => scene::Shape::Hexagon,
            3 => scene::Shape::Triangle,
            _ => scene::Shape::Rectangle,
        }
    }

    fn cap_to_u8(cap: scene::Cap) -> u8 {
        match cap {
            scene::Cap::Arrow => 1,
            scene::Cap::Round => 2,
            scene::Cap::None => u8::MAX,
        }
    }

    fn u8_to_cap(int: u8) -> scene::Cap {
        match int {
            1 => scene::Cap::Arrow,
            2 => scene::Cap::Round,
            _ => scene::Cap::None,
        }
    }

    #[derive(Serialize, Deserialize)]
    struct Project {
        id: Id,
        title: String,
        scenes: Vec<Scene>,
    }

    #[derive(Serialize, Deserialize)]
    struct Scene {
        id: Id,
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
