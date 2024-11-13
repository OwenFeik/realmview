use uuid::Uuid;

use super::{timestamp_s, Conn};
use crate::utils::{err, format_uuid, generate_uuid, parse_uuid, Res};

pub struct Scene {
    pub uuid: Uuid,
    pub project: Uuid,
    pub updated_time: std::time::SystemTime,
    pub title: String,
    pub thumbnail: Option<String>,
}

impl Scene {
    pub fn updated_timestamp(&self) -> u64 {
        self.updated_time
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    pub async fn get_by_uuid(conn: &mut Conn, uuid: Uuid) -> Res<Self> {
        match Self::lookup(conn, uuid).await? {
            Some(record) => Ok(record),
            None => err("Scene does not exist."),
        }
    }

    async fn lookup(conn: &mut Conn, uuid: Uuid) -> Res<Option<Self>> {
        if let Some(row) = lookup(conn, uuid).await? {
            Ok(Some(Self::try_from(row)?))
        } else {
            Ok(None)
        }
    }

    pub async fn list_for_project(conn: &mut Conn, project: Uuid) -> Res<Vec<Self>> {
        list_for_project(conn, project)
            .await?
            .into_iter()
            .map(Self::try_from)
            .collect()
    }

    pub async fn update_or_create(conn: &mut Conn, scene: &mut scene::Scene) -> Res<Self> {
        let (scene_uuid, exists) = match Self::lookup(conn, scene.uuid).await? {
            Some(record) if record.project == scene.project => (record.uuid, true),
            _ => (generate_uuid(), false), // Record doesn't exist or in other project.
        };
        scene.uuid = scene_uuid;

        if exists {
            update(conn, scene_uuid, &scene.title)
                .await
                .and_then(Self::try_from)
        } else {
            create(conn, scene_uuid, scene.project, &scene.title)
                .await
                .and_then(Self::try_from)
        }
    }

    pub async fn set_thumbnail(conn: &mut Conn, uuid: Uuid, thumbnail: &str) -> Res<()> {
        set_thumbnail(conn, uuid, thumbnail).await
    }
}

impl TryFrom<SceneRow> for Scene {
    type Error = String;

    fn try_from(value: SceneRow) -> Res<Self> {
        Ok(Scene {
            uuid: parse_uuid(&value.uuid)?,
            project: parse_uuid(&value.project)?,
            updated_time: std::time::UNIX_EPOCH
                + std::time::Duration::from_secs(value.updated_time as u64),
            title: value.title,
            thumbnail: value.thumbnail,
        })
    }
}

#[derive(sqlx::FromRow)]
struct SceneRow {
    uuid: String,
    project: String,
    updated_time: i64,
    title: String,
    thumbnail: Option<String>,
}

async fn lookup(conn: &mut Conn, uuid: Uuid) -> Res<Option<SceneRow>> {
    let uuid_string = format_uuid(uuid);
    sqlx::query_as!(
        SceneRow,
        "
        SELECT uuid, project, updated_time, title, thumbnail
        FROM scenes WHERE uuid = ?; 
        ",
        uuid_string
    )
    .fetch_optional(conn)
    .await
    .map_err(|e| e.to_string())
}

async fn list_for_project(conn: &mut Conn, project: Uuid) -> Res<Vec<SceneRow>> {
    let uuid_string = format_uuid(project);
    sqlx::query_as!(
        SceneRow,
        "
        SELECT uuid, project, updated_time, title, thumbnail
        FROM scenes WHERE project = ?;
        ",
        uuid_string
    )
    .fetch_all(conn)
    .await
    .map_err(|e| e.to_string())
}

async fn set_thumbnail(conn: &mut Conn, uuid: Uuid, thumbnail: &str) -> Res<()> {
    let uuid_string = format_uuid(uuid);
    sqlx::query!(
        "UPDATE scenes SET thumbnail = ?1 WHERE uuid = ?2;",
        thumbnail,
        uuid_string
    )
    .execute(conn)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

async fn update(conn: &mut Conn, uuid: Uuid, title: &str) -> Res<SceneRow> {
    let timestamp = timestamp_s();
    let uuid_string = format_uuid(uuid);
    sqlx::query_as!(
        SceneRow,
        "UPDATE scenes SET updated_time = ?1, title = ?2 WHERE uuid = ?3 RETURNING *;",
        timestamp,
        title,
        uuid_string,
    )
    .fetch_one(conn)
    .await
    .map_err(|e| e.to_string())
}

async fn create(conn: &mut Conn, uuid: Uuid, project: Uuid, title: &str) -> Res<SceneRow> {
    let uuid = format_uuid(uuid);
    let project = format_uuid(project);
    let updated_time = timestamp_s();
    sqlx::query_as!(
        SceneRow,
        "
        INSERT INTO scenes (uuid, project, updated_time, title)
        VALUES (?1, ?2, ?3, ?4) RETURNING *;
        ",
        uuid,
        project,
        updated_time,
        title,
    )
    .fetch_one(conn)
    .await
    .map_err(|e| e.to_string())
}
