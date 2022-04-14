use sqlx::SqlitePool;

use self::layer::LayerRecord;
use self::scene_record::SceneRecord;
use self::sprite::SpriteRecord;

#[derive(sqlx::FromRow)]
pub struct Project {
    pub id: i64,
    pub user: i64,
    pub title: String,
}

impl Project {
    async fn new(pool: &SqlitePool, user: i64) -> anyhow::Result<Project> {
        sqlx::query_as("INSERT INTO projects (user, title) VALUES (?1, ?2) RETURNING *;")
            .bind(user)
            .bind("Untitled")
            .fetch_one(pool)
            .await
            .map_err(|_| anyhow::anyhow!("Database error."))
    }

    async fn load(pool: &SqlitePool, id: i64) -> anyhow::Result<Project> {
        let res = sqlx::query_as("SELECT * FROM projects WHERE id = ?1;")
            .bind(id)
            .fetch_optional(pool)
            .await;

        match res {
            Ok(Some(p)) => Ok(p),
            Ok(None) => Err(anyhow::anyhow!("Project not found.")),
            Err(_) => Err(anyhow::anyhow!("Database error.")),
        }
    }

    pub async fn get_or_create(
        pool: &SqlitePool,
        id: Option<i64>,
        user: i64,
    ) -> anyhow::Result<Project> {
        match id {
            Some(id) => Project::load(pool, id).await,
            None => Project::new(pool, user).await,
        }
    }

    pub async fn update_scene(
        &self,
        pool: &SqlitePool,
        scene: scene::Scene,
    ) -> anyhow::Result<SceneRecord> {
        let s = scene_record::SceneRecord::get_or_create(pool, scene.id, self.id).await?;

        for layer in scene.layers.iter() {
            let l = LayerRecord::get_or_create(pool, layer, s.id).await?;

            for sprite in layer.sprites.iter() {
                SpriteRecord::save(pool, sprite, l.id).await?;
            }
        }

        Ok(s)
    }
}

mod scene_record {
    use sqlx::SqlitePool;

    use super::{layer::LayerRecord, sprite::SpriteRecord};

    #[derive(sqlx::FromRow)]
    pub struct SceneRecord {
        pub id: i64,
        pub project: i64,
        pub title: String,
    }

    impl SceneRecord {
        pub async fn load(pool: &SqlitePool, id: i64) -> anyhow::Result<SceneRecord> {
            sqlx::query_as("SELECT * FROM scenes WHERE id = ?1;")
                .bind(id)
                .fetch_one(pool)
                .await
                .map_err(|_| anyhow::anyhow!("Failed to find scene."))
        }

        async fn create(pool: &SqlitePool, project: i64) -> anyhow::Result<SceneRecord> {
            sqlx::query_as("INSERT INTO scenes (project, title) VALUES (?1, ?2) RETURNING *;")
                .bind(project)
                .bind("Untitled")
                .fetch_one(pool)
                .await
                .map_err(|_| anyhow::anyhow!("Failed to create scene."))
        }

        pub async fn get_or_create(
            pool: &SqlitePool,
            id: Option<i64>,
            project: i64,
        ) -> anyhow::Result<SceneRecord> {
            match id {
                Some(id) => SceneRecord::load(pool, id).await,
                None => SceneRecord::create(pool, project).await,
            }
        }

        pub async fn load_scene(&self, pool: &SqlitePool) -> anyhow::Result<scene::Scene> {
            let layers = LayerRecord::load_scene_layers(pool, self.id).await?;
            let mut sprites = SpriteRecord::sprites_for_layers(pool, &layers).await?;
            let mut layers = layers
                .iter()
                .map(|lr| lr.to_layer())
                .collect::<Vec<scene::Layer>>();

            while let Some(s) = sprites.pop() {
                if let Some(l) = layers.iter_mut().find(|l| l.canonical_id == Some(s.layer)) {
                    l.add_sprite(s.to_sprite());
                }
            }

            Ok(scene::Scene {
                id: Some(self.id),
                layers,
                project: Some(self.project),
                holding: scene::HeldObject::None,
            })
        }
    }
}

mod layer {
    use sqlx::SqlitePool;

    #[derive(sqlx::FromRow)]
    pub struct LayerRecord {
        pub id: i64,
        scene: i64,
        title: String,
        z: i64,
    }

    impl LayerRecord {
        pub fn to_layer(&self) -> scene::Layer {
            scene::Layer {
                local_id: self.id,
                canonical_id: Some(self.id),
                title: self.title.clone(),
                z: self.z as i32,
                sprites: vec![],
                z_min: 0,
                z_max: 0,
            }
        }

        async fn load(pool: &SqlitePool, id: i64) -> anyhow::Result<LayerRecord> {
            sqlx::query_as("SELECT * FROM layers WHERE id = ?1;")
                .bind(id)
                .fetch_one(pool)
                .await
                .map_err(|_| anyhow::anyhow!("Failed to load layer."))
        }

        async fn create(
            pool: &SqlitePool,
            layer: &scene::Layer,
            scene: i64,
        ) -> anyhow::Result<LayerRecord> {
            sqlx::query_as("INSERT INTO layers (scene, title, z) VALUES (?1, ?2, ?3) RETURNING *;")
                .bind(scene)
                .bind(&layer.title)
                .bind(layer.z as i64)
                .fetch_one(pool)
                .await
                .map_err(|_| anyhow::anyhow!("Failed to create layer."))
        }

        pub async fn get_or_create(
            pool: &SqlitePool,
            layer: &scene::Layer,
            scene: i64,
        ) -> anyhow::Result<LayerRecord> {
            match layer.canonical_id {
                Some(id) => LayerRecord::load(pool, id).await,
                None => LayerRecord::create(pool, layer, scene).await,
            }
        }

        pub async fn load_scene_layers(
            pool: &SqlitePool,
            scene: i64,
        ) -> anyhow::Result<Vec<LayerRecord>> {
            sqlx::query_as("SELECT * FROM layers WHERE scene = ?1;")
                .bind(scene)
                .fetch_all(pool)
                .await
                .map_err(|_| anyhow::anyhow!("Failed to load scene layers."))
        }
    }
}

mod sprite {
    use sqlx::Row;

    use super::layer::LayerRecord;

    // Can't use RETURNING * with SQLite due to bug with REAL columns, which is
    // relevant to the Sprite type because x, y, w, h are all REAL. May be
    // resolved in a future SQLite version but error persists in 3.38.0.
    // see: https://github.com/launchbadge/sqlx/issues/1596

    #[derive(sqlx::FromRow)]
    pub struct SpriteRecord {
        id: i64,
        pub layer: i64,
        media: i64,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        z: i64,
    }

    impl SpriteRecord {
        fn from_sprite(sprite: &scene::Sprite, layer: i64, id: i64) -> Self {
            Self {
                id,
                layer,
                media: sprite.texture,
                x: sprite.rect.x,
                y: sprite.rect.y,
                w: sprite.rect.w,
                h: sprite.rect.h,
                z: sprite.z as i64,
            }
        }

        pub fn to_sprite(&self) -> scene::Sprite {
            scene::Sprite {
                rect: scene::Rect::new(self.x, self.y, self.w, self.h),
                z: self.z as i32,
                texture: self.media,
                local_id: self.id,
                canonical_id: Some(self.id),
            }
        }

        // NB: will panic if called with a scene::Sprite with canonical_id None
        async fn update_from(
            pool: &sqlx::SqlitePool,
            sprite: &scene::Sprite,
            layer: i64,
        ) -> anyhow::Result<()> {
            sqlx::query("UPDATE sprites SET (layer, media, x, y, w, h, z) = (?1, ?2, ?3, ?4, ?5, ?6, ?7) WHERE id = ?8;")
                .bind(layer)
                .bind(sprite.texture)
                .bind(sprite.rect.x)
                .bind(sprite.rect.y)
                .bind(sprite.rect.w)
                .bind(sprite.rect.h)
                .bind(sprite.z)
                .bind(sprite.canonical_id.unwrap())
                .execute(pool)
                .await
                .map(|_| ())
                .map_err(|_| anyhow::anyhow!("Failed to update sprite."))
        }

        async fn create_from(
            pool: &sqlx::SqlitePool,
            sprite: &scene::Sprite,
            layer: i64,
        ) -> anyhow::Result<i64> {
            sqlx::query(
                "INSERT INTO sprites (layer, media, x, y, w, h, z) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) RETURNING id;"
            )
                .bind(layer)
                .bind(sprite.texture)
                .bind(sprite.rect.x)
                .bind(sprite.rect.y)
                .bind(sprite.rect.w)
                .bind(sprite.rect.h)
                .bind(sprite.z)
                .fetch_one(pool)
                .await
                .map(|row: sqlx::sqlite::SqliteRow| row.get(0))
                .map_err(|_| anyhow::anyhow!("Failed to create sprite."))
        }

        pub async fn save(
            pool: &sqlx::SqlitePool,
            sprite: &scene::Sprite,
            layer: i64,
        ) -> anyhow::Result<SpriteRecord> {
            let id = match sprite.canonical_id {
                Some(id) => {
                    SpriteRecord::update_from(pool, sprite, layer).await?;
                    id
                }
                None => SpriteRecord::create_from(pool, sprite, layer).await?,
            };
            Ok(SpriteRecord::from_sprite(sprite, layer, id))
        }

        pub async fn sprites_for_layers(
            pool: &sqlx::SqlitePool,
            layers: &[LayerRecord],
        ) -> anyhow::Result<Vec<SpriteRecord>> {
            sqlx::query_as(&format!(
                "SELECT * FROM sprites WHERE layer IN ({});",
                layers
                    .iter()
                    .map(|l| l.id.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ))
            .fetch_all(pool)
            .await
            .map_err(|_| anyhow::anyhow!("Failed to load sprite list."))
        }
    }
}
