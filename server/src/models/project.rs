use std::path::PathBuf;

use sqlx::prelude::FromRow;
use uuid::Uuid;

use super::{timestamp_s, timestamp_to_system, Conn, Scene, User};
use crate::{
    fs::{join_relative_path, write_file, SAVES},
    utils::{err, format_uuid, generate_uuid, parse_uuid, Res},
};

#[derive(Debug)]
pub struct Project {
    pub uuid: Uuid,
    pub user: Uuid,
    pub updated_time: std::time::SystemTime,
    pub title: String,
}

impl Project {
    pub const MAX_TITLE_LENGTH: usize = 256;

    pub async fn save(
        conn: &mut Conn,
        user: &User,
        mut project: scene::Project,
    ) -> Res<(Self, Vec<Scene>)> {
        let record: Self = Self::update_or_create(conn, &mut project, user.uuid).await?;
        let mut scenes = Vec::new();
        for scene in &mut project.scenes {
            scene.project = record.uuid;
            scenes.push(Scene::update_or_create(conn, scene).await?);
        }
        record.remove_deleted_scenes(conn, &scenes).await;

        let path = record.save_path(&user.username);
        let data = scene::serde::serialise(&project)?;
        write_file(path, data).await?;

        Ok((record, scenes))
    }

    pub async fn load_file(&self, conn: &mut Conn) -> Res<Vec<u8>> {
        let user = User::get_by_uuid(conn, self.user).await?;
        let path = self.save_path(&user.username);
        tokio::fs::read(path).await.map_err(|e| e.to_string())
    }

    pub async fn load(&self, conn: &mut Conn) -> Res<scene::Project> {
        scene::serde::deserialise(&self.load_file(conn).await?)
    }

    fn save_path(&self, username: &str) -> PathBuf {
        const FILE_EXTENSION: &str = "rvp";
        join_relative_path(
            &SAVES,
            format!(
                "{}/{}.{}",
                &username,
                format_uuid(self.uuid),
                FILE_EXTENSION
            ),
        )
    }

    pub async fn get_by_uuid(conn: &mut Conn, uuid: Uuid) -> Res<Self> {
        match Self::lookup(conn, uuid).await? {
            Some(project) => Ok(project),
            None => err("Project does not exist."),
        }
    }

    pub async fn lookup(conn: &mut Conn, uuid: Uuid) -> Res<Option<Self>> {
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

    pub fn validate_title(title: &str) -> Res<()> {
        if title.len() > Self::MAX_TITLE_LENGTH {
            Err(format!(
                "Title too long ({} characters), max length is {}.",
                title.len(),
                Self::MAX_TITLE_LENGTH
            ))
        } else {
            Ok(())
        }
    }

    pub async fn create(conn: &mut Conn, user: &User, title: &str) -> Res<Self> {
        Self::validate_title(title)?;
        let record = create_project(conn, generate_uuid(), user.uuid, title)
            .await
            .and_then(Self::try_from)?;
        let data =
            scene::serde::serialise(&scene::Project::titled(record.uuid, title.to_string()))?;
        write_file(record.save_path(&user.username), data).await?;
        Ok(record)
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
            update_project(conn, project_uuid, &project.title)
                .await
                .and_then(Self::try_from)
        } else {
            create_project(conn, project_uuid, user, &project.title)
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
            updated_time: timestamp_to_system(value.updated_time),
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

async fn create_project(conn: &mut Conn, uuid: Uuid, user: Uuid, title: &str) -> Res<ProjectRow> {
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

async fn update_project(conn: &mut Conn, uuid: Uuid, title: &str) -> Res<ProjectRow> {
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
    use crate::{
        fs::database_connection,
        models::{Scene, User},
    };

    #[tokio::test]
    async fn test_remove_deleted_scenes() {
        let conn = &mut database_connection().await.unwrap();
        let user = User::generate(conn).await.unwrap();

        // Create a fresh project and save it.
        let project = Project::create(conn, &user, "projecttitle").await.unwrap();
        let mut proj = scene::Project::new(project.uuid);
        proj.new_scene();
        proj.new_scene();
        proj.new_scene();
        proj.new_scene();
        let (project, scenes) = Project::save(conn, &user, proj).await.unwrap();

        // Load the project, delete a couple of scenes and re-save.
        let project = Project::get_by_uuid(conn, project.uuid).await.unwrap();
        let mut proj = project.load(conn).await.unwrap();
        assert_eq!(proj.scenes.len(), 4);
        proj.delete_scene(scenes.get(1).unwrap().uuid).unwrap();
        proj.delete_scene(scenes.get(3).unwrap().uuid).unwrap();
        Project::save(conn, &user, proj).await.unwrap();

        // Load the project again and check that the scenes are gone.
        let proj = Project::get_by_uuid(conn, project.uuid)
            .await
            .unwrap()
            .load(conn)
            .await
            .unwrap();
        assert_eq!(proj.scenes.len(), 2);
        assert!(proj.get_scene(scenes.first().unwrap().uuid).is_some());
        assert!(proj.get_scene(scenes.get(1).unwrap().uuid).is_none());
        assert!(proj.get_scene(scenes.get(2).unwrap().uuid).is_some());
        assert!(proj.get_scene(scenes.get(3).unwrap().uuid).is_none());
        assert_eq!(
            Scene::list_for_project(conn, proj.uuid)
                .await
                .unwrap()
                .len(),
            2
        );
    }
}
