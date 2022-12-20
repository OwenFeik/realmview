use anyhow::anyhow;
use sqlx::{Row, SqliteConnection};

use self::layer::LayerRecord;
pub use self::scene_record::SceneRecord;
use self::sprite::SpriteRecord;
use crate::crypto;

const RECORD_KEY_LENGTH: usize = 16;

#[derive(sqlx::FromRow)]
pub struct Project {
    pub id: i64,
    pub project_key: String,
    pub user: i64,
    pub title: String,
}

impl Project {
    const DEFAULT_TITLE: &'static str = "Untitled";
    const MAX_TITLE_LENGTH: usize = 256;

    pub async fn new(
        conn: &mut SqliteConnection,
        user: i64,
        title: &str,
    ) -> anyhow::Result<Project> {
        if title.len() > Self::MAX_TITLE_LENGTH {
            return Err(anyhow!("Title too long."));
        }

        sqlx::query_as(
            "INSERT INTO projects (project_key, user, title) VALUES (?1, ?2, ?3) RETURNING *;",
        )
        .bind(crypto::random_hex_string(RECORD_KEY_LENGTH)?)
        .bind(user)
        .bind(title)
        .fetch_one(conn)
        .await
        .map_err(|e| anyhow!(e))
    }

    pub async fn delete(self, conn: &mut SqliteConnection) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            DELETE FROM sprites WHERE scene IN (
                SELECT id FROM scenes WHERE project = ?1
            );
            DELETE FROM scenes WHERE project = ?1;
            DELETE FROM projects WHERE id = ?1;
        "#,
        )
        .bind(self.id)
        .execute(conn)
        .await
        .map_err(|e| anyhow!(e))?;
        Ok(())
    }

    pub async fn load(conn: &mut SqliteConnection, id: i64) -> anyhow::Result<Project> {
        let res = sqlx::query_as("SELECT * FROM projects WHERE id = ?1;")
            .bind(id)
            .fetch_optional(conn)
            .await;

        match res {
            Ok(Some(p)) => Ok(p),
            Ok(None) => Err(anyhow::anyhow!("Project not found.")),
            Err(_) => Err(anyhow::anyhow!("Database error.")),
        }
    }

    pub async fn list(conn: &mut SqliteConnection, user: i64) -> anyhow::Result<Vec<Project>> {
        sqlx::query_as("SELECT * FROM projects WHERE user = ?1;")
            .bind(user)
            .fetch_all(conn)
            .await
            .map_err(|e| anyhow::anyhow!(format!("Database error: {}", e)))
    }

    pub async fn get_or_create(
        conn: &mut SqliteConnection,
        id: Option<i64>,
        user: i64,
    ) -> anyhow::Result<Project> {
        match id {
            Some(id) => Project::load(conn, id).await,
            None => Project::new(conn, user, Self::DEFAULT_TITLE).await,
        }
    }

    pub async fn get_by_key(
        conn: &mut SqliteConnection,
        project_key: &str,
    ) -> anyhow::Result<Project> {
        sqlx::query_as("SELECT * FROM projects WHERE project_key = ?1;")
            .bind(project_key)
            .fetch_one(conn)
            .await
            .map_err(|e| anyhow!(e))
    }

    pub async fn update_title(
        &mut self,
        conn: &mut SqliteConnection,
        title: String,
    ) -> anyhow::Result<()> {
        let res = sqlx::query("UPDATE projects SET title = ?1 WHERE id = ?2;")
            .bind(&title)
            .bind(self.id)
            .execute(conn)
            .await;

        if let Err(e) = res {
            return Err(anyhow::anyhow!(format!(
                "Failed to update project title: {e}"
            )));
        }

        self.title = title;
        Ok(())
    }

    pub async fn update_scene(
        &self,
        conn: &mut SqliteConnection,
        scene: scene::Scene,
        scene_title: String,
    ) -> anyhow::Result<SceneRecord> {
        let s = scene_record::SceneRecord::get_or_create(
            conn,
            scene.id,
            self.id,
            scene_title,
            scene.w(),
            scene.h(),
            scene.fog.bytes(),
        )
        .await?;

        for layer in scene.removed_layers.iter() {
            for sprite in &layer.sprites {
                SpriteRecord::delete(conn, sprite.id, s.id).await?;
            }
        }

        for layer in &scene.layers {
            for sprite in &layer.removed_sprites {
                SpriteRecord::delete(conn, sprite.id, s.id).await?;
            }

            let l = LayerRecord::update_or_create(conn, layer, s.id).await?;

            for sprite in &layer.sprites {
                SpriteRecord::save(conn, sprite, l.id, s.id).await?;
            }
        }

        Ok(s)
    }

    pub async fn list_scenes(
        &self,
        conn: &mut SqliteConnection,
    ) -> anyhow::Result<Vec<SceneRecord>> {
        SceneRecord::project_scenes(conn, self.id).await
    }

    pub async fn scene_owner(conn: &mut SqliteConnection, scene_key: &str) -> anyhow::Result<i64> {
        let row = sqlx::query("SELECT user FROM scenes LEFT JOIN projects ON scenes.project = projects.id WHERE scenes.scene_key = ?1;")
            .bind(scene_key)
            .fetch_one(conn)
            .await
            .map_err(|_| anyhow!("Scene not found."))?;

        Ok(row.get(0))
    }

    pub async fn set_scene_thumbnail(
        conn: &mut SqliteConnection,
        scene_key: &str,
        thumbnail: &str,
    ) -> anyhow::Result<()> {
        sqlx::query("UPDATE scenes SET thumbnail = ?1 WHERE scene_key = ?2;")
            .bind(thumbnail)
            .bind(scene_key)
            .execute(conn)
            .await
            .map_err(|e| anyhow!("Database error: {e}"))?;
        Ok(())
    }

    pub async fn update_scene_title(
        &self,
        conn: &mut SqliteConnection,
        scene_key: &str,
        title: &str,
    ) -> anyhow::Result<()> {
        sqlx::query("UPDATE scenes SET title = ?1 WHERE project = ?2 AND scene_key = ?3;")
            .bind(title)
            .bind(self.id)
            .bind(scene_key)
            .execute(conn)
            .await
            .map_err(|e| anyhow!("Database error: {e}"))?;
        Ok(())
    }

    pub async fn delete_scene(
        conn: &mut SqliteConnection,
        user: i64,
        scene_key: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            DELETE FROM sprites WHERE scene IN (
                SELECT scenes.id FROM scenes
                LEFT JOIN projects ON scenes.project = projects.id
                WHERE scenes.scene_key = ?1 AND projects.user = ?2
            );
            DELETE FROM scenes WHERE id IN (
                SELECT scenes.id FROM scenes
                LEFT JOIN projects ON scenes.project = projects.id
                WHERE scenes.scene_key = ?1 AND projects.user = ?2
            );
        "#,
        )
        .bind(scene_key)
        .bind(user)
        .execute(conn)
        .await
        .map_err(|e| anyhow!(e))?;
        Ok(())
    }
}

mod scene_record {
    use anyhow::anyhow;
    use sqlx::{Row, SqliteConnection};

    use super::{layer::LayerRecord, sprite::SpriteRecord, RECORD_KEY_LENGTH};
    use crate::crypto;

    #[derive(sqlx::FromRow)]
    pub struct SceneRecord {
        pub id: i64,
        pub scene_key: String,
        pub project: i64,
        pub title: String,
        pub w: u32,
        pub h: u32,
        pub thumbnail: String,
        pub fog: Vec<u8>,
    }

    impl SceneRecord {
        pub async fn load(conn: &mut SqliteConnection, id: i64) -> anyhow::Result<SceneRecord> {
            sqlx::query_as("SELECT * FROM scenes WHERE id = ?1;")
                .bind(id)
                .fetch_one(conn)
                .await
                .map_err(|_| anyhow!("Failed to find scene."))
        }

        pub async fn load_from_key(
            conn: &mut SqliteConnection,
            scene_key: &str,
        ) -> anyhow::Result<SceneRecord> {
            sqlx::query_as("SELECT * FROM scenes WHERE scene_key = ?1")
                .bind(scene_key)
                .fetch_one(conn)
                .await
                .map_err(|_| anyhow!("Failed to find scene."))
        }

        async fn create(
            conn: &mut SqliteConnection,
            project: i64,
            title: &str,
            width: u32,
            height: u32,
            fog: Vec<u8>,
        ) -> anyhow::Result<SceneRecord> {
            sqlx::query_as(
                "INSERT INTO scenes (scene_key, project, title, w, h, fog) VALUES (?1, ?2, ?3, ?4, ?5, ?6) RETURNING *;",
            )
            .bind(crypto::random_hex_string(RECORD_KEY_LENGTH)?)
            .bind(project)
            .bind(title)
            .bind(width)
            .bind(height)
            .bind(fog)
            .fetch_one(conn)
            .await
            .map_err(|e| anyhow!("Failed to create scene: {e}"))
        }

        pub async fn get_or_create(
            conn: &mut SqliteConnection,
            id: Option<i64>,
            project: i64,
            title: String,
            width: u32,
            height: u32,
            fog: Vec<u8>,
        ) -> anyhow::Result<SceneRecord> {
            let mut record = match id {
                Some(id) => SceneRecord::load(conn, id).await,
                None => SceneRecord::create(conn, project, &title, width, height, fog).await,
            }?;

            if record.title != title {
                record.update_title(conn, &title).await?;
                record.title = title;
            }
            Ok(record)
        }

        async fn update_title(
            &self,
            conn: &mut SqliteConnection,
            title: &str,
        ) -> anyhow::Result<()> {
            sqlx::query("UPDATE scenes SET title = ?1 WHERE id = ?2;")
                .bind(title)
                .bind(self.id)
                .execute(conn)
                .await
                .map_err(|_| anyhow!("Failed to update scene title."))?;
            Ok(())
        }

        pub async fn load_scene(
            &self,
            conn: &mut SqliteConnection,
        ) -> anyhow::Result<scene::Scene> {
            let layers = LayerRecord::load_scene_layers(conn, self.id).await?;
            let mut sprites = SpriteRecord::load_scene_sprites(conn, self.id).await?;
            let mut layers = layers
                .iter()
                .map(|lr| lr.to_layer())
                .collect::<Vec<scene::Layer>>();

            while let Some(s) = sprites.pop() {
                if let Some(l) = layers.iter_mut().find(|l| l.id == s.layer) {
                    l.add_sprite(s.to_sprite());
                }
            }

            let mut scene = scene::Scene::new_with_layers(layers);
            scene.id = Some(self.id);
            scene.title = Some(self.title.clone());
            scene.project = Some(self.project);
            scene.fog = scene::Fog::from_bytes(self.w, self.h, &self.fog);
            Ok(scene)
        }

        pub async fn user(&self, conn: &mut SqliteConnection) -> anyhow::Result<i64> {
            sqlx::query(
                "SELECT user FROM scenes LEFT JOIN projects ON scenes.project = projects.id WHERE scenes.id = ?1;"
            )
                .bind(self.id)
                .fetch_one(conn)
                .await
                .map(|row: sqlx::sqlite::SqliteRow| row.get(0))
                .map_err(|e| anyhow!("Failed to load scene user: {e}"))
        }

        pub async fn project_scenes(
            conn: &mut SqliteConnection,
            project: i64,
        ) -> anyhow::Result<Vec<SceneRecord>> {
            sqlx::query_as("SELECT * FROM scenes WHERE project = ?1;")
                .bind(project)
                .fetch_all(conn)
                .await
                .map_err(|e| anyhow::anyhow!(format!("Failed to load scene list: {e}")))
        }
    }
}

mod layer {
    use anyhow::anyhow;
    use sqlx::SqliteConnection;

    #[derive(Debug, sqlx::FromRow)]
    pub struct LayerRecord {
        pub id: i64,
        scene: i64,
        title: String,
        z: i64,
        visible: bool,
        locked: bool,
    }

    impl LayerRecord {
        pub fn to_layer(&self) -> scene::Layer {
            scene::Layer {
                id: self.id,
                title: self.title.clone(),
                z: self.z as i32,
                visible: self.visible,
                locked: self.locked,
                sprites: vec![],
                removed_sprites: vec![],
                z_min: 0,
                z_max: 0,
            }
        }

        async fn load(
            conn: &mut SqliteConnection,
            scene: i64,
            id: i64,
        ) -> anyhow::Result<LayerRecord> {
            sqlx::query_as("SELECT * FROM layers WHERE scene = ?1 AND id = ?2;")
                .bind(scene)
                .bind(id)
                .fetch_one(conn)
                .await
                .map_err(|_| anyhow!("Failed to load layer."))
        }

        async fn create(
            conn: &mut SqliteConnection,
            layer: &scene::Layer,
            scene: i64,
        ) -> anyhow::Result<LayerRecord> {
            sqlx::query_as("INSERT INTO layers (id, scene, title, z, visible, locked) VALUES (?1, ?2, ?3, ?4, ?5, ?6) RETURNING *;")
                .bind(layer.id)
                .bind(scene)
                .bind(&layer.title)
                .bind(layer.z as i64)
                .bind(layer.visible)
                .bind(layer.locked)
                .fetch_one(conn)
                .await
                .map_err(|_| anyhow!("Failed to create layer."))
        }

        pub async fn delete(
            conn: &mut SqliteConnection,
            id: i64,
            scene: i64,
        ) -> anyhow::Result<()> {
            sqlx::query("DELETE FROM layers WHERE id = ?1 AND scene = ?2;")
                .bind(id)
                .bind(scene)
                .execute(conn)
                .await
                .map(|_| ())
                .map_err(|e| anyhow!("Failed to delete layer: {e}"))
        }

        pub async fn update_or_create(
            conn: &mut SqliteConnection,
            layer: &scene::Layer,
            scene: i64,
        ) -> anyhow::Result<LayerRecord> {
            LayerRecord::delete(conn, layer.id, scene).await.ok();
            LayerRecord::create(conn, layer, scene).await
        }

        pub async fn load_scene_layers(
            pool: &mut SqliteConnection,
            scene: i64,
        ) -> anyhow::Result<Vec<LayerRecord>> {
            sqlx::query_as("SELECT * FROM layers WHERE scene = ?1;")
                .bind(scene)
                .fetch_all(pool)
                .await
                .map_err(|e| anyhow!("Failed to load scene layers: {e}"))
        }
    }
}

mod sprite {
    use anyhow::anyhow;
    use sqlx::{Row, SqliteConnection};

    use crate::models::Media;

    // Can't use RETURNING * with SQLite due to bug with REAL columns, which is
    // relevant to the Sprite type because x, y, w, h are all REAL. May be
    // resolved in a future SQLite version but error persists in 3.38.0.
    // see: https://github.com/launchbadge/sqlx/issues/1596
    //
    // Confirmed not working in sqlx = 0.6.2

    #[derive(Debug, sqlx::FromRow)]
    pub struct SpriteRecord {
        id: i64,
        scene: i64,
        pub layer: i64,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        z: i64,
        shape: Option<u8>,
        stroke: Option<f32>,
        media_key: Option<String>,
        points: Option<Vec<u8>>,
        r: Option<f32>,
        g: Option<f32>,
        b: Option<f32>,
        a: Option<f32>,
        drawing_type: Option<u8>,
        cap_start: Option<u8>,
        cap_end: Option<u8>,
    }

    impl SpriteRecord {
        fn from_sprite(sprite: &scene::Sprite, layer: i64, scene: i64) -> Self {
            let (cap_start, cap_end) = if let scene::SpriteVisual::Drawing(drawing) = &sprite.visual
            {
                (
                    Some(Self::cap_to_u8(drawing.cap_start)),
                    Some(Self::cap_to_u8(drawing.cap_end)),
                )
            } else {
                (None, None)
            };

            let mut record = Self {
                id: sprite.id,
                scene,
                layer,
                x: sprite.rect.x,
                y: sprite.rect.y,
                w: sprite.rect.w,
                h: sprite.rect.h,
                z: sprite.z as i64,
                shape: sprite.visual.shape().map(Self::shape_to_u8),
                stroke: sprite.visual.stroke(),
                media_key: sprite.visual.texture().map(Media::id_to_key),
                points: sprite
                    .visual
                    .drawing()
                    .map(|p| p.points.data.iter().flat_map(|f| f.to_be_bytes()).collect()),
                r: sprite.visual.colour().map(|c| c[0]),
                g: sprite.visual.colour().map(|c| c[1]),
                b: sprite.visual.colour().map(|c| c[2]),
                a: sprite.visual.colour().map(|c| c[3]),
                drawing_type: sprite
                    .visual
                    .drawing()
                    .map(|d| Self::drawing_type_to_u8(d.drawing_type)),
                cap_start,
                cap_end,
            };

            if let Some([r, g, b, a]) = sprite.visual.colour() {
                record.r = Some(r);
                record.g = Some(g);
                record.b = Some(b);
                record.a = Some(a);
            }

            record
        }

        fn shape_to_u8(shape: scene::SpriteShape) -> u8 {
            match shape {
                scene::SpriteShape::Ellipse => 1,
                scene::SpriteShape::Hexagon => 2,
                scene::SpriteShape::Triangle => 3,
                scene::SpriteShape::Rectangle => 4,
            }
        }

        fn u8_to_shape(int: u8) -> scene::SpriteShape {
            match int {
                1 => scene::SpriteShape::Ellipse,
                2 => scene::SpriteShape::Hexagon,
                3 => scene::SpriteShape::Triangle,
                _ => scene::SpriteShape::Rectangle,
            }
        }

        fn drawing_type_to_u8(drawing_type: scene::SpriteDrawingType) -> u8 {
            match drawing_type {
                scene::SpriteDrawingType::Freehand => 1,
                scene::SpriteDrawingType::Line => 2,
            }
        }

        fn u8_to_drawing_type(int: u8) -> scene::SpriteDrawingType {
            match int {
                1 => scene::SpriteDrawingType::Freehand,
                2 => scene::SpriteDrawingType::Line,
                _ => scene::SpriteDrawingType::Freehand,
            }
        }

        fn cap_to_u8(cap: scene::SpriteCap) -> u8 {
            match cap {
                scene::SpriteCap::Arrow => 1,
                scene::SpriteCap::Round => 2,
                scene::SpriteCap::None => u8::MAX,
            }
        }

        fn u8_to_cap(int: u8) -> scene::SpriteCap {
            match int {
                1 => scene::SpriteCap::Arrow,
                3 => scene::SpriteCap::Round,
                _ => scene::SpriteCap::None,
            }
        }

        fn visual(&self) -> Option<scene::SpriteVisual> {
            if let Some(points) = &self.points {
                Some(scene::SpriteVisual::Drawing(scene::SpriteDrawing {
                    drawing_type: Self::u8_to_drawing_type(self.drawing_type?),
                    points: scene::PointVector::from(
                        points
                            .chunks_exact(32 / 8)
                            .map(|b| f32::from_be_bytes([b[0], b[1], b[2], b[3]]))
                            .collect(),
                    ),
                    stroke: self.stroke?,
                    colour: [self.r?, self.g?, self.b?, self.a?],
                    cap_start: Self::u8_to_cap(self.cap_start?),
                    cap_end: Self::u8_to_cap(self.cap_end?),
                    finished: true,
                }))
            } else if let Some(key) = &self.media_key {
                Some(scene::SpriteVisual::Texture {
                    shape: Self::u8_to_shape(self.shape?),
                    id: Media::key_to_id(key).ok()?,
                })
            } else {
                Some(scene::SpriteVisual::Solid {
                    shape: Self::u8_to_shape(self.shape?),
                    stroke: self.stroke?,
                    colour: [self.r?, self.g?, self.b?, self.a?],
                })
            }
        }

        pub fn to_sprite(&self) -> scene::Sprite {
            let mut sprite = scene::Sprite::new(self.id, self.visual());
            sprite.rect = scene::Rect::new(self.x, self.y, self.w, self.h);
            sprite.z = self.z as i32;
            sprite
        }

        async fn create_from(
            conn: &mut SqliteConnection,
            sprite: &scene::Sprite,
            layer: i64,
            scene: i64,
        ) -> anyhow::Result<i64> {
            let record = Self::from_sprite(sprite, layer, scene);
            sqlx::query(
                r#"
                INSERT INTO sprites (
                    id, scene, layer, x, y, w, h, z, shape, stroke, media_key, points, r, g, b, a, drawing_type, cap_start, cap_end
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19
                ) RETURNING id;
                "#,
            )
            .bind(record.id)
            .bind(record.scene)
            .bind(record.layer)
            .bind(record.x)
            .bind(record.y)
            .bind(record.w)
            .bind(record.h)
            .bind(record.z)
            .bind(record.shape)
            .bind(record.stroke)
            .bind(record.media_key)
            .bind(record.points)
            .bind(record.r)
            .bind(record.g)
            .bind(record.b)
            .bind(record.a)
            .bind(record.drawing_type)
            .bind(record.cap_start)
            .bind(record.cap_end)
            .fetch_one(conn)
            .await
            .map(|row: sqlx::sqlite::SqliteRow| row.get(0))
            .map_err(|e| anyhow!("Failed to create sprite: {e}"))
        }

        pub async fn delete(
            conn: &mut SqliteConnection,
            id: i64,
            scene: i64,
        ) -> anyhow::Result<()> {
            sqlx::query("DELETE FROM sprites WHERE id = ?1 AND scene = ?2;")
                .bind(id)
                .bind(scene)
                .execute(conn)
                .await
                .map(|_| ())
                .map_err(|e| anyhow!("Failed to delete sprite: {e}"))
        }

        pub async fn save(
            conn: &mut SqliteConnection,
            sprite: &scene::Sprite,
            layer: i64,
            scene: i64,
        ) -> anyhow::Result<SpriteRecord> {
            SpriteRecord::delete(conn, sprite.id, scene).await.ok();
            let id = SpriteRecord::create_from(conn, sprite, layer, scene).await?;
            Ok(SpriteRecord::from_sprite(sprite, layer, id))
        }

        pub async fn load_scene_sprites(
            conn: &mut SqliteConnection,
            scene: i64,
        ) -> anyhow::Result<Vec<SpriteRecord>> {
            sqlx::query_as("SELECT * FROM sprites WHERE scene = ?1;")
                .bind(scene)
                .fetch_all(conn)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to load sprite list: {e}."))
        }
    }
}
