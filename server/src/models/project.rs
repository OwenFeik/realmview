use sqlx::SqlitePool;

use self::layer::Layer;
use self::scene_record::Scene;
use self::sprite::Sprite;

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

    pub async fn update_scene(&self, pool: &SqlitePool, scene: scene::Scene) -> anyhow::Result<()> {
        let s = Scene::get_or_create(pool, scene.id, self.id).await?;

        for layer in scene.layers.iter() {
            let l = Layer::get_or_create(pool, layer, s.id).await?;

            for sprite in layer.sprites.iter() {
                Sprite::save(pool, sprite, l.id).await?;
            }
        }

        Ok(())
    }
}

mod scene_record {
    use sqlx::SqlitePool;

    #[derive(sqlx::FromRow)]
    pub struct Scene {
        pub id: i64,
        pub project: i64,
        pub title: String,
    }

    impl Scene {
        async fn load(pool: &SqlitePool, id: i64) -> anyhow::Result<Scene> {
            sqlx::query_as("SELECT * FROM scenes WHERE id = ?1;")
                .bind(id)
                .fetch_one(pool)
                .await
                .map_err(|_| anyhow::anyhow!("Failed to find scene"))
        }

        async fn create(pool: &SqlitePool, project: i64) -> anyhow::Result<Scene> {
            sqlx::query_as("INSERT INTO scenes (project, title) VALUES (?1, ?2) RETURNING *;")
                .bind(project)
                .bind("Untitled")
                .fetch_one(pool)
                .await
                .map_err(|_| anyhow::anyhow!("Failed to create scene"))
        }

        pub async fn get_or_create(
            pool: &SqlitePool,
            id: Option<i64>,
            project: i64,
        ) -> anyhow::Result<Scene> {
            match id {
                Some(id) => Scene::load(pool, id).await,
                None => Scene::create(pool, project).await,
            }
        }
    }
}

mod layer {
    use sqlx::SqlitePool;

    #[derive(sqlx::FromRow)]
    pub struct Layer {
        pub id: i64,
        scene: i64,
        title: String,
        z: i64,
    }

    impl Layer {
        async fn load(pool: &SqlitePool, id: i64) -> anyhow::Result<Layer> {
            sqlx::query_as("SELECT * FROM layers WHERE id = ?1;")
                .bind(id)
                .fetch_one(pool)
                .await
                .map_err(|_| anyhow::anyhow!("Failed to load layer"))
        }

        async fn create(
            pool: &SqlitePool,
            layer: &scene::Layer,
            scene: i64,
        ) -> anyhow::Result<Layer> {
            sqlx::query_as("INSERT INTO layers (scene, title, z) VALUES (?1, ?2, ?3) RETURNING *;")
                .bind(scene)
                .bind(&layer.title)
                .bind(layer.z as i64)
                .fetch_one(pool)
                .await
                .map_err(|_| anyhow::anyhow!("Failed to create layer"))
        }

        pub async fn get_or_create(
            pool: &SqlitePool,
            layer: &scene::Layer,
            scene: i64,
        ) -> anyhow::Result<Layer> {
            match layer.canonical_id {
                Some(id) => Layer::load(pool, id).await,
                None => Layer::create(pool, layer, scene).await,
            }
        }
    }
}

mod sprite {
    #[derive(sqlx::FromRow)]
    pub struct Sprite {
        id: i64,
        layer: i64,
        media: i64,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    }

    impl Sprite {
        async fn update_from(
            pool: &sqlx::SqlitePool,
            sprite: &scene::Sprite,
            layer: i64,
        ) -> anyhow::Result<Sprite> {
            sqlx::query_as("UPDATE sprites SET (layer, media, x, y, w, h) = (?1, ?2, ?3, ?4, ?5, ?6) RETURNING *;")
                .bind(layer)
                .bind(sprite.texture)
                .bind(sprite.rect.x)
                .bind(sprite.rect.y)
                .bind(sprite.rect.w)
                .bind(sprite.rect.h)
                .fetch_one(pool)
                .await
                .map_err(|_| anyhow::anyhow!("Failed to update sprite"))
        }

        async fn create_from(
            pool: &sqlx::SqlitePool,
            sprite: &scene::Sprite,
            layer: i64,
        ) -> anyhow::Result<Sprite> {
            sqlx::query_as(
                "INSERT INTO sprites (layer, media, x, y, w, h) VALUES (?1, ?2, ?3, ?4, ?5, ?6) RETURNING *;"
            )
                .bind(layer)
                .bind(sprite.texture)
                .bind(sprite.rect.x)
                .bind(sprite.rect.y)
                .bind(sprite.rect.w)
                .bind(sprite.rect.h)
                .fetch_one(pool)
                .await
                .map_err(|_| anyhow::anyhow!("Failed to create sprite"))
        }

        pub async fn save(
            pool: &sqlx::SqlitePool,
            sprite: &scene::Sprite,
            layer: i64,
        ) -> anyhow::Result<Sprite> {
            match sprite.canonical_id {
                Some(_) => Sprite::update_from(pool, sprite, layer).await,
                None => Sprite::create_from(pool, sprite, layer).await,
            }
        }
    }
}
