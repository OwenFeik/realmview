use uuid::Uuid;

use super::{timestamp_s, Conn, Project, Scene, User};
use crate::{
    fs::{join_relative_path, SAVES},
    utils::{err, format_uuid, generate_uuid, Res},
};

impl Scene {
    pub async fn get_by_uuid(conn: &mut Conn, uuid: Uuid) -> Res<Self> {
        match Self::lookup(conn, uuid).await? {
            Some(record) => Ok(record),
            None => err("Scene does not exist."),
        }
    }

    async fn lookup(conn: &mut Conn, uuid: Uuid) -> Res<Option<Self>> {
        sqlx::query_as(
            "
            SELECT (uuid, project, updated_time, title, thumbnail)
            FROM scenes WHERE uuid = ?1; 
            ",
        )
        .bind(format_uuid(uuid))
        .fetch_optional(conn)
        .await
        .map_err(|e| e.to_string())
    }

    pub async fn set_thumbnail(conn: &mut Conn, uuid: Uuid, thumbnail: &str) -> Res<()> {
        sqlx::query("UPDATE scenes SET thumbnail = ?1 WHERE uuid = ?2;")
            .bind(thumbnail)
            .bind(format_uuid(uuid))
            .execute(conn)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn update_or_create(
        conn: &mut Conn,
        project: Uuid,
        scene: &mut scene::Scene,
    ) -> Res<Self> {
        let (scene_uuid, exists) = match Self::lookup(conn, scene.uuid).await? {
            Some(record) if record.project == project => (record.uuid, true),
            _ => (generate_uuid(), false), // Record doesn't exist or in other project.
        };
        scene.uuid = scene_uuid;

        if exists {
            sqlx::query_as(
                "UPDATE scenes SET updated_time = ?1, title = ?2 WHERE uuid = ?3 RETURNING *;",
            )
            .bind(timestamp_s())
            .bind(&scene.title)
            .bind(format_uuid(scene_uuid))
            .fetch_one(conn)
            .await
            .map_err(|e| e.to_string())
        } else {
            sqlx::query_as(
                "
                INSERT INTO scenes (uuid, project, updated_time, title)
                VALUES (?1, ?2, ?3, ?4) RETURNING *;",
            )
            .bind(format_uuid(scene_uuid))
            .bind(format_uuid(project))
            .bind(timestamp_s())
            .bind(&scene.title)
            .fetch_one(conn)
            .await
            .map_err(|e| e.to_string())
        }
    }
}

impl Project {
    pub async fn save(
        conn: &mut Conn,
        owner: Uuid,
        mut project: scene::Project,
    ) -> Res<(Self, Vec<Scene>)> {
        let user = User::get_by_uuid(conn, owner).await?;

        let record: Self = Self::update_or_create(conn, &mut project, owner).await?;
        let mut scenes = Vec::new();
        for scene in &mut project.scenes {
            scenes.push(Scene::update_or_create(conn, record.uuid, scene).await?);
        }
        record.remove_deleted_scenes(conn, &scenes).await;

        let data = scene::serde::serialise(&project)?;
        let path = join_relative_path(&SAVES, user.relative_save_path());
        tokio::fs::write(path, data)
            .await
            .map_err(|e| e.to_string())?;

        Ok((record, scenes))
    }

    async fn remove_deleted_scenes(&self, conn: &mut Conn, scenes: &[Scene]) {
        let scene_ids = scenes
            .iter()
            .map(|scene| format!("'{}'", format_uuid(scene.uuid)))
            .collect::<Vec<String>>()
            .join(", ");
        sqlx::query(&format!(
            "DELETE FROM scenes WHERE project = ?1 AND uuid NOT IN ({})",
            scene_ids
        ))
        .bind(self.uuid)
        .execute(conn)
        .await
        .ok();
    }

    pub async fn load(&self) -> Res<scene::Project> {
        todo!("Load project associated this DB record")
    }

    pub async fn get_by_uuid(conn: &mut Conn, uuid: Uuid) -> Res<Self> {
        match Self::lookup(conn, uuid).await? {
            Some(record) => Ok(record),
            None => err("Project does not exist."),
        }
    }

    async fn lookup(conn: &mut Conn, uuid: Uuid) -> Res<Option<Self>> {
        sqlx::query_as(
            "
            SELECT (uuid, user, updated_time, title)
            FROM projects WHERE uuid = ?1;
            ",
        )
        .bind(format_uuid(uuid))
        .fetch_optional(conn)
        .await
        .map_err(|e| e.to_string())
    }

    pub async fn list_scenes(&self, conn: &mut Conn) -> Res<Vec<Scene>> {
        sqlx::query_as(
            "
            SELECT (uuid, project, updated_time, title, thumbnail)
            FROM scenes WHERE project = ?1;
            ",
        )
        .bind(format_uuid(self.uuid))
        .fetch_all(conn)
        .await
        .map_err(|e| e.to_string())
    }

    pub async fn list_for_user(conn: &mut Conn, user: Uuid) -> Res<Vec<Self>> {
        sqlx::query_as(
            "
            SELECT (uuid, user, updated_time, title)
            FROM projects WHERE user = ?1;
            ",
        )
        .bind(format_uuid(user))
        .fetch_all(conn)
        .await
        .map_err(|e| e.to_string())
    }

    pub async fn create(conn: &mut Conn, user: Uuid, title: &str) -> Res<Self> {
        sqlx::query_as(
            "
        INSERT INTO projects (uuid, user, updated_time, title)
        VALUES (?1, ?2, ?3, ?4) RETURNING *;
        ",
        )
        .bind(format_uuid(generate_uuid()))
        .bind(format_uuid(user))
        .bind(timestamp_s())
        .bind(title)
        .fetch_one(conn)
        .await
        .map_err(|e| e.to_string())
    }

    pub async fn delete(self, conn: &mut Conn) -> Res<()> {
        sqlx::query("DELETE FROM projects WHERE uuid = ?1;")
            .bind(format_uuid(self.uuid))
            .execute(conn)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn for_scene(conn: &mut Conn, scene: Uuid) -> Res<Self> {
        sqlx::query_as(
            "
            SELECT p.uuid, p.user, p.updated_time, p.title
            FROM project p, scene
            WHERE scene.uuid = ?1 AND p.uuid = scene.project
            ",
        )
        .bind(format_uuid(scene))
        .fetch_one(conn)
        .await
        .map_err(|e| e.to_string())
    }

    async fn update_or_create(
        conn: &mut Conn,
        project: &mut scene::Project,
        user: Uuid,
    ) -> Res<Self> {
        let (project_uuid, exists) = match Self::lookup(conn, project.uuid).await? {
            Some(record) if record.user == user => (record.uuid, true),
            _ => (generate_uuid(), false), // Record doesn't exist or owned by another.
        };
        project.uuid = project_uuid;

        if exists {
            sqlx::query_as(
                "UPDATE projects SET updated_time = ?1, title = ?2 WHERE uuid = ?3 RETURNING *;",
            )
            .bind(timestamp_s())
            .bind(&project.title)
            .bind(format_uuid(project_uuid))
            .fetch_one(conn)
            .await
            .map_err(|e| e.to_string())
        } else {
            sqlx::query_as(
                r#"
                INSERT INTO projects (uuid, user, updated_time, title)
                VALUES (?1, ?2, ?3, ?4) RETURNING *;
                "#,
            )
            .bind(format_uuid(project_uuid))
            .bind(format_uuid(user))
            .bind(timestamp_s())
            .bind(&project.title)
            .fetch_one(conn)
            .await
            .map_err(|e| e.to_string())
        }
    }
}
