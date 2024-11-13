use sqlx::prelude::FromRow;
use uuid::Uuid;

use super::{timestamp_s, Conn, Scene, User};
use crate::{
    fs::{join_relative_path, SAVES},
    utils::{err, format_uuid, generate_uuid, parse_uuid, Res},
};

pub struct Project {
    pub uuid: Uuid,
    pub user: Uuid,
    pub updated_time: std::time::SystemTime,
    pub title: String,
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
            scene.project = record.uuid;
            scenes.push(Scene::update_or_create(conn, scene).await?);
        }
        record.remove_deleted_scenes(conn, &scenes).await;

        let data = scene::serde::serialise(&project)?;
        let path = join_relative_path(&SAVES, user.relative_save_path());
        tokio::fs::write(path, data)
            .await
            .map_err(|e| e.to_string())?;

        Ok((record, scenes))
    }

    pub async fn load(&self) -> Res<scene::Project> {
        todo!("Load project associated this DB record")
    }

    pub async fn get_by_uuid(conn: &mut Conn, uuid: Uuid) -> Res<Self> {
        match Self::lookup(conn, uuid).await? {
            Some(project) => Ok(project),
            None => err("Project does not exist."),
        }
    }

    async fn lookup(conn: &mut Conn, uuid: Uuid) -> Res<Option<Self>> {
        if let Some(row) = lookup(conn, uuid).await? {
            Ok(Some(Self::try_from(row)?))
        } else {
            Ok(None)
        }
    }

    pub async fn list_scenes(&self, conn: &mut Conn) -> Res<Vec<Scene>> {
        Scene::list_for_project(conn, self.uuid).await
    }

    pub async fn list_for_user(conn: &mut Conn, user: Uuid) -> Res<Vec<Self>> {
        list_for_user(conn, user)
            .await?
            .into_iter()
            .map(Self::try_from)
            .collect()
    }

    pub async fn create(conn: &mut Conn, user: Uuid, title: &str) -> Res<Self> {
        create(conn, generate_uuid(), user, title)
            .await
            .and_then(Self::try_from)
    }

    pub async fn delete(self, conn: &mut Conn) -> Res<()> {
        let uuid = format_uuid(self.uuid);
        sqlx::query!("DELETE FROM projects WHERE uuid = ?1;", uuid)
            .execute(conn)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn for_scene(conn: &mut Conn, scene: Uuid) -> Res<Self> {
        for_scene(conn, scene).await.and_then(Self::try_from)
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
            update(conn, project_uuid, &project.title)
                .await
                .and_then(Self::try_from)
        } else {
            create(conn, project_uuid, user, &project.title)
                .await
                .and_then(Self::try_from)
        }
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
        .bind(format_uuid(self.uuid))
        .execute(conn)
        .await
        .ok();
    }
}

impl TryFrom<ProjectRow> for Project {
    type Error = String;

    fn try_from(value: ProjectRow) -> Res<Self> {
        Ok(Self {
            uuid: parse_uuid(&value.uuid)?,
            user: parse_uuid(&value.user)?,
            updated_time: std::time::UNIX_EPOCH
                + std::time::Duration::from_secs(value.updated_time as u64),
            title: value.title,
        })
    }
}

#[derive(FromRow)]
pub struct ProjectRow {
    uuid: String,
    user: String,
    updated_time: i64,
    title: String,
}

async fn create(conn: &mut Conn, uuid: Uuid, user: Uuid, title: &str) -> Res<ProjectRow> {
    let uuid = format_uuid(uuid);
    let user = format_uuid(user);
    let updated_time = timestamp_s();
    sqlx::query_as!(
        ProjectRow,
        "
        INSERT INTO projects (uuid, user, updated_time, title)
        VALUES (?1, ?2, ?3, ?4) RETURNING *;
        ",
        uuid,
        user,
        updated_time,
        title
    )
    .fetch_one(conn)
    .await
    .map_err(|e| e.to_string())
}

async fn update(conn: &mut Conn, uuid: Uuid, title: &str) -> Res<ProjectRow> {
    let updated_time = timestamp_s();
    let uuid = format_uuid(uuid);
    sqlx::query_as!(
        ProjectRow,
        "UPDATE projects SET updated_time = ?1, title = ?2 WHERE uuid = ?3 RETURNING *;",
        updated_time,
        title,
        uuid
    )
    .fetch_one(conn)
    .await
    .map_err(|e| e.to_string())
}

async fn lookup(conn: &mut Conn, uuid: Uuid) -> Res<Option<ProjectRow>> {
    let uuid = format_uuid(uuid);
    sqlx::query_as!(
        ProjectRow,
        "
        SELECT uuid, user, updated_time, title
        FROM projects WHERE uuid = ?1;
        ",
        uuid
    )
    .fetch_optional(conn)
    .await
    .map_err(|e| e.to_string())
}

async fn for_scene(conn: &mut Conn, scene: Uuid) -> Res<ProjectRow> {
    let scene = format_uuid(scene);
    sqlx::query_as!(
        ProjectRow,
        "
        SELECT p.uuid, p.user, p.updated_time, p.title
        FROM projects p, scenes
        WHERE scenes.uuid = ?1 AND p.uuid = scenes.project
        ",
        scene
    )
    .fetch_one(conn)
    .await
    .map_err(|e| e.to_string())
}

async fn list_for_user(conn: &mut Conn, user: Uuid) -> Res<Vec<ProjectRow>> {
    let user = format_uuid(user);
    sqlx::query_as!(
        ProjectRow,
        "
        SELECT uuid, user, updated_time, title
        FROM projects WHERE user = ?1;
        ",
        user
    )
    .fetch_all(conn)
    .await
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod test {
    use super::Project;
    use crate::models::User;

    #[tokio::test]
    async fn test_remove_deleted_scenes() {
        let pool = &crate::fs::initialise_database().await.unwrap();
        let conn = &mut pool.acquire().await.unwrap();
        let user = User::register(pool, "testuser", "salt", "hashpassword", "recoverykey")
            .await
            .unwrap();
        let project = Project::create(conn, user, "projecttitle").await.unwrap();
    }
}
