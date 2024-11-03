use uuid::Uuid;

use super::{timestamp_s, Conn, Project, Scene};
use crate::utils::{format_uuid, generate_uuid, Res};

impl Scene {
    pub async fn load_by_uuid(conn: &mut Conn, uuid: Uuid) -> Res<Self> {
        sqlx::query_as(
            "
            SELECT (uuid, project, updated_time, title, thumbnail)
            FROM scenes WHERE uuid = ?1; 
            ",
        )
        .bind(format_uuid(uuid))
        .fetch_one(conn)
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
}

impl Project {
    pub async fn load_by_uuid(conn: &mut Conn, uuid: Uuid) -> Res<Self> {
        sqlx::query_as(
            "
            SELECT (uuid, user, updated_time, title)
            FROM projects WHERE uuid = ?1;
            ",
        )
        .bind(format_uuid(uuid))
        .fetch_one(conn)
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
}

mod db {
    use sqlx::SqliteConnection;
    use uuid::Uuid;

    use super::{Project, Scene};
    use crate::utils::{format_uuid, timestamp_s, Res};

    async fn update_database(
        conn: &mut SqliteConnection,
        project: &scene::Project,
        user: i64,
    ) -> Res<()> {
        update_or_create_project_record(conn, project, user).await?;
        for scene in &project.scenes {
            update_or_create_scene_record(conn, scene).await?;
        }
        Ok(())
    }

    async fn remove_deleted_scenes(conn: &mut SqliteConnection, project: &scene::Project) {
        let scene_ids = project
            .scenes
            .iter()
            .map(|scene| format!("'{}'", scene.uuid.simple()))
            .collect::<Vec<String>>()
            .join(", ");
        sqlx::query(&format!(
            "DELETE FROM scenes WHERE project = ?1 AND uuid NOT IN ({})",
            scene_ids
        ))
        .bind(project.uuid)
        .execute(conn)
        .await
        .ok();
    }

    async fn update_project_record(
        conn: &mut SqliteConnection,
        uuid: Uuid,
    ) -> Res<Option<Project>> {
        match sqlx::query_as(
            "UPDATE projects SET updated_time = ?1, title = ?2 WHERE uuid = ?3 RETURNING *;",
        )
        .bind(format_uuid(uuid))
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
    ) -> Res<Project> {
        let record = update_project_record(conn, project.uuid).await?;
        let now = timestamp_s().unwrap_or(0) as i64;
        if let Some(record) = record {
            Ok(record)
        } else {
            sqlx::query_as(
                r#"
                INSERT INTO projects (uuid, user, updated_time, title)
                VALUES (?1, ?2, ?3, ?4) RETURNING *;
                "#,
            )
            .bind(format_uuid(project.uuid))
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
    ) -> Res<Option<Project>> {
        match sqlx::query_as(
            "UPDATE scenes SET updated_time = ?1, title = ?2 WHERE id = ?3 RETURNING *;",
        )
        .bind(updated_timestamp())
        .bind(&scene.title)
        .bind(format_uuid(project))
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
        scene: &scene::Scene,
    ) -> Res<Option<Scene>> {
        match sqlx::query_as(
            "UPDATE scenes SET updated_time = ?1, title = ?2 WHERE uuid = ?3 RETURNING *;",
        )
        .bind(updated_timestamp())
        .bind(scene.title.clone())
        .bind(scene.uuid)
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
