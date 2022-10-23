use std::convert::Infallible;

use anyhow::anyhow;
use bytes::BufMut;
use futures::TryStreamExt;
use ring::digest;
use sqlx::{Row, SqlitePool};
use warp::{
    multipart::{FormData, Part},
    Filter,
};

use super::response::{as_result, Binary};
use super::{with_db, with_session, with_val};
use crate::crypto::to_hex_string_unsized;
use crate::models::{Media, User};

#[derive(serde_derive::Serialize)]
struct UploadResponse {
    message: String,
    success: bool,
    media_key: Option<String>,
    url: String,
}

impl UploadResponse {
    fn new(key: Option<String>, url: String) -> UploadResponse {
        UploadResponse {
            message: String::from("Uploaded successfully."),
            success: true,
            media_key: key,
            url,
        }
    }
}

#[derive(Debug)]
enum ImageRole {
    Media,
    Thumbnail(String),
}

struct UploadImage {
    role: ImageRole,
    data: Option<Vec<u8>>,
    title: String,
    ext: String,
    key: Option<String>,
}

impl UploadImage {
    const DEFAULT_TITLE: &'static str = "untitled";
    const DEFAULT_EXT: &'static str = "png";

    fn new() -> anyhow::Result<Self> {
        Ok(UploadImage {
            role: ImageRole::Media,
            data: None,
            title: Self::DEFAULT_TITLE.to_owned(),
            ext: Self::DEFAULT_EXT.to_owned(),
            key: None,
        })
    }

    fn ensure_key(&mut self) -> anyhow::Result<()> {
        if self.key.is_none() {
            self.key = Some(Media::generate_key()?);
        }
        Ok(())
    }

    /// Make sure to ensure_key() first.
    fn relative_path(&self, user: &User) -> anyhow::Result<String> {
        match &self.role {
            ImageRole::Media => Ok(format!(
                "{}/{}.{}",
                &user.relative_dir(),
                self.key.as_ref().unwrap(),
                self.ext
            )),
            ImageRole::Thumbnail(scene_key) => Ok(format!(
                "{}/thumbnails/{}.{}",
                &user.relative_dir(),
                scene_key,
                self.ext
            )),
        }
    }

    fn real_path(&self, content_dir: &str, user: &User) -> anyhow::Result<String> {
        let relative_path = self.relative_path(user)?;
        Ok(format!("{}/{}", content_dir, &relative_path))
    }

    async fn create_directory(&self, content_dir: &str, user: &User) -> anyhow::Result<()> {
        let mut directory = user.upload_dir(content_dir);
        if matches!(self.role, ImageRole::Thumbnail(..)) {
            directory.push_str("/thumbnails");
        }

        tokio::fs::create_dir_all(directory)
            .await
            .map_err(|e| anyhow!("Failed to create directory: {e}"))
    }

    async fn write_file(&self, content_dir: &str, user: &User) -> anyhow::Result<()> {
        self.create_directory(content_dir, user).await?;

        match &self.data {
            Some(data) => tokio::fs::write(self.real_path(content_dir, user)?, data)
                .await
                .map_err(|e| anyhow!("Failed to write file: {e}")),
            None => Err(anyhow!("No image data provided.")),
        }
    }

    fn file_hash(&self) -> anyhow::Result<String> {
        if let Some(data) = self.data.as_ref() {
            hash_file(data)
        } else {
            Err(anyhow!("No image data provided."))
        }
    }

    async fn save_thumbnail(
        &self,
        pool: &SqlitePool,
        user: &User,
        content_dir: &str,
    ) -> anyhow::Result<UploadResponse> {
        let scene_key = match &self.role {
            ImageRole::Thumbnail(key) => key,
            _ => return Err(anyhow!("No scene ID provided.")),
        };

        let mut conn = pool
            .acquire()
            .await
            .map_err(|e| anyhow!("Failed to get database connection: {e}"))?;

        if crate::models::Project::scene_owner(&mut conn, scene_key).await? != user.id {
            return Err(anyhow!("User does not own scene."));
        }

        self.write_file(content_dir, user).await?;

        let relative_path = self.relative_path(user)?;
        crate::models::Project::set_scene_thumbnail(&mut conn, scene_key, &relative_path)
            .await
            .ok();

        Ok(UploadResponse::new(
            None,
            format!("/static/{}", &relative_path),
        ))
    }

    async fn save_media(
        &mut self,
        pool: &SqlitePool,
        user: &User,
        content_dir: &str,
    ) -> anyhow::Result<UploadResponse> {
        let hash = self.file_hash()?;

        if let Some(existing_title) = file_exists(pool, user.id, &hash).await? {
            return {
                if existing_title == self.title {
                    Err(anyhow!("File already uploaded."))
                } else {
                    Err(anyhow!("File already uploaded as {}", &existing_title))
                }
            };
        }

        self.ensure_key()?;
        let relative_path = self.relative_path(user)?;
        self.write_file(content_dir, user).await?;
        match Media::create(
            pool,
            self.key.as_ref().unwrap(),
            user.id,
            &relative_path,
            &self.title,
            &hash,
        )
        .await
        {
            Ok(media) => {
                let url = format!("/static/{}", &relative_path);
                Ok(UploadResponse::new(Some(media.media_key), url))
            }
            Err(e) => {
                // Remove file as part of cleanup.
                tokio::fs::remove_file(&self.real_path(content_dir, user)?)
                    .await
                    .ok();
                Err(anyhow!("Database error: {e}"))
            }
        }
    }

    async fn save(
        &mut self,
        pool: &SqlitePool,
        user: &User,
        content_dir: &str,
    ) -> anyhow::Result<UploadResponse> {
        match self.role {
            ImageRole::Media => self.save_media(pool, user, content_dir).await,
            ImageRole::Thumbnail(..) => self.save_thumbnail(pool, user, content_dir).await,
        }
    }
}

fn hash_file(raw: &[u8]) -> anyhow::Result<String> {
    to_hex_string_unsized(digest::digest(&digest::SHA256, raw).as_ref())
}

async fn file_exists(pool: &SqlitePool, user: i64, hash: &str) -> anyhow::Result<Option<String>> {
    let row_opt = sqlx::query("SELECT title FROM media WHERE user = ?1 AND hashed_value = ?2;")
        .bind(user)
        .bind(hash)
        .fetch_optional(pool)
        .await?;

    if let Some(row) = row_opt {
        match row.try_get(0) {
            Ok(s) => Ok(Some(s)),
            Err(_) => Err(anyhow::anyhow!("Database error.")),
        }
    } else {
        Ok(None)
    }
}

async fn collect_part(part: Part) -> anyhow::Result<Vec<u8>> {
    part.stream()
        .try_fold(Vec::new(), |mut vec, data| {
            vec.put(data);
            async move { Ok(vec) }
        })
        .await
        .map_err(|e| anyhow!("Failed to read part: {e}"))
}

async fn upload(
    pool: SqlitePool,
    session_key: String,
    content_dir: String,
    form: FormData,
) -> Result<impl warp::Reply, Infallible> {
    let user = match User::get_by_session(&pool, &session_key).await {
        Ok(Some(user)) => user,
        _ => return Binary::result_failure("Invalid session."),
    };

    let parts: Vec<Part> = match form.try_collect().await {
        Ok(v) => v,
        Err(_) => return Binary::result_failure("Upload failed."),
    };

    let mut upload = match UploadImage::new() {
        Ok(u) => u,
        Err(e) => return Binary::from_error(e),
    };

    for p in parts {
        match p.name() {
            "thumbnail" => match collect_part(p).await.map(String::from_utf8) {
                Ok(Ok(scene_key)) => upload.role = ImageRole::Thumbnail(scene_key),
                _ => return Binary::result_failure("Bad thumbnail scene ID."),
            },
            "image" => {
                upload.ext = match p.content_type() {
                    Some(mime) => match mime {
                        "image/png" => "png",
                        "image/jpeg" => "jpeg",
                        _ => {
                            return Binary::result_failure(&format!(
                                "Unsupported image type: {}",
                                mime
                            ))
                        }
                    }
                    .to_owned(),
                    None => return Binary::result_failure("Missing content type."),
                };

                match p.filename() {
                    Some(s) => upload.title = s.to_owned(),
                    None => upload.title = format!("untitled.{}", upload.ext),
                };

                match collect_part(p).await {
                    Ok(data) => upload.data = Some(data),
                    Err(e) => return Binary::from_error(e),
                }
            }
            _ => (),
        }
    }

    match upload.save(&pool, &user, &content_dir).await {
        Ok(resp) => as_result(&resp, warp::http::StatusCode::OK),
        Err(e) => Binary::result_failure(&e.to_string()),
    }
}

pub fn filter(
    pool: SqlitePool,
    content_dir: String,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("upload")
        .and(warp::post())
        .and(with_db(pool))
        .and(with_session())
        .and(with_val(content_dir))
        .and(warp::multipart::form())
        .and_then(upload)
}
