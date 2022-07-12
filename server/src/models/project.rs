use sqlx::SqliteConnection;

use crate::crypto;

use self::layer::LayerRecord;
pub use self::scene_record::SceneRecord;
use self::sprite::SpriteRecord;

const RECORD_KEY_LENGTH: usize = 16;

#[derive(sqlx::FromRow)]
pub struct Project {
    pub id: i64,
    pub project_key: String,
    pub user: i64,
    pub title: String,
}

impl Project {
    async fn new(conn: &mut SqliteConnection, user: i64) -> anyhow::Result<Project> {
        sqlx::query_as(
            "INSERT INTO projects (project_key, user, title) VALUES (?1, ?2, ?3) RETURNING *;",
        )
        .bind(crypto::random_hex_string(RECORD_KEY_LENGTH)?)
        .bind(user)
        .bind("Untitled")
        .fetch_one(conn)
        .await
        .map_err(|e| anyhow::anyhow!(format!("Database error: {}", e)))
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
            None => Project::new(conn, user).await,
        }
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
            scene.w,
            scene.h,
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
}

mod scene_record {
    use anyhow::anyhow;
    use sqlx::{Row, SqliteConnection};

    use crate::crypto;

    use super::{layer::LayerRecord, sprite::SpriteRecord, RECORD_KEY_LENGTH};

    #[derive(sqlx::FromRow)]
    pub struct SceneRecord {
        pub id: i64,
        pub scene_key: String,
        pub project: i64,
        pub title: String,
        pub w: u32,
        pub h: u32,
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
        ) -> anyhow::Result<SceneRecord> {
            sqlx::query_as(
                "INSERT INTO scenes (scene_key, project, title, w, h) VALUES (?1, ?2, ?3, ?4, ?5) RETURNING *;",
            )
            .bind(crypto::random_hex_string(RECORD_KEY_LENGTH)?)
            .bind(project)
            .bind(title)
            .bind(width)
            .bind(height)
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
        ) -> anyhow::Result<SceneRecord> {
            let mut record = match id {
                Some(id) => SceneRecord::load(conn, id).await,
                None => SceneRecord::create(conn, project, &title, width, height).await,
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
            scene.w = self.w;
            scene.h = self.h;
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

    #[derive(sqlx::FromRow)]
    pub struct SpriteRecord {
        id: i64,
        pub layer: i64,
        media_key: Option<String>,
        r: Option<f32>,
        g: Option<f32>,
        b: Option<f32>,
        a: Option<f32>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        z: i64,
    }

    impl SpriteRecord {
        fn from_sprite(sprite: &scene::Sprite, layer: i64, id: i64) -> Self {
            let mut record = Self {
                id,
                layer,
                media_key: None,
                r: None,
                g: None,
                b: None,
                a: None,
                x: sprite.rect.x,
                y: sprite.rect.y,
                w: sprite.rect.w,
                h: sprite.rect.h,
                z: sprite.z as i64,
            };

            match sprite.visual {
                scene::SpriteVisual::Colour([r, g, b, a]) => {
                    record.r = Some(r);
                    record.g = Some(g);
                    record.b = Some(b);
                    record.a = Some(a);
                },
                scene::SpriteVisual::Texture(id) => {
                    record.media_key = Some(Media::id_to_key(id)); 
                }
            };
            
            record
        }

        fn visual(&self) -> Option<scene::SpriteVisual> {
            if let Some(key) = &self.media_key {
                Some(scene::SpriteVisual::Texture(Media::key_to_id(key).ok()?))
            }
            else {
                Some(scene::SpriteVisual::Colour([self.r?, self.g?, self.b?, self.a?]))
            }
        }

        pub fn to_sprite(&self) -> scene::Sprite {
            let mut sprite = scene::Sprite::new(self.id, self.visual());
            sprite.set_rect(scene::Rect::new(self.x, self.y, self.w, self.h));
            sprite.z = self.z as i32;
            sprite
        }

        async fn create_from(
            conn: &mut SqliteConnection,
            sprite: &scene::Sprite,
            layer: i64,
            scene: i64,
        ) -> anyhow::Result<i64> {
            sqlx::query(
                r#"
                INSERT INTO sprites (
                    id, scene, layer, media_key, r, g, b, a, x, y, w, h, z
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13
                ) RETURNING id;
                "#
            )
                .bind(sprite.id)
                .bind(scene)
                .bind(layer)
                .bind(sprite.visual.texture().map(Media::id_to_key))
                .bind(sprite.visual.colour().map(|c| c[0]))
                .bind(sprite.visual.colour().map(|c| c[1]))
                .bind(sprite.visual.colour().map(|c| c[2]))
                .bind(sprite.visual.colour().map(|c| c[3]))
                .bind(sprite.rect.x)
                .bind(sprite.rect.y)
                .bind(sprite.rect.w)
                .bind(sprite.rect.h)
                .bind(sprite.z)
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
                .map_err(|_| anyhow::anyhow!("Failed to load sprite list."))
        }
    }
}
