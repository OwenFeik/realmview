use std::path::PathBuf;

use actix_multipart::{Field, Multipart};
use actix_web::web;
use anyhow::anyhow;
use futures::{StreamExt, TryStreamExt};
use ring::digest;
use sqlx::SqlitePool;

use super::{res_failure, res_json, Res};
use crate::{
    crypto::to_hex_string_unsized,
    models::{Media, User},
    req::e500,
    utils::join_relative_path,
    CONTENT,
};

// Maximum total size of media a single use can upload, in bytes
const UPLOAD_LIMIT: usize = 10 * 1024 * 1024 * 1024; // 10 GB

pub fn routes() -> actix_web::Scope {
    web::scope("/upload").default_service(web::route().to(upload))
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

    fn size(&self) -> usize {
        match &self.data {
            Some(data) => data.len(),
            None => 0,
        }
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

    fn real_path(&self, user: &User) -> anyhow::Result<PathBuf> {
        Ok(join_relative_path(
            CONTENT.as_path(),
            self.relative_path(user)?,
        ))
    }

    async fn create_directory(&self, user: &User) -> anyhow::Result<()> {
        let mut directory = user.upload_dir(&CONTENT.to_string_lossy());
        if matches!(self.role, ImageRole::Thumbnail(..)) {
            directory.push_str("/thumbnails");
        }

        tokio::fs::create_dir_all(directory)
            .await
            .map_err(|e| anyhow!("Failed to create directory: {e}"))
    }

    async fn write_file(&self, user: &User) -> anyhow::Result<()> {
        self.create_directory(user).await?;

        match &self.data {
            Some(data) => {
                let path = self.real_path(user)?;
                tokio::fs::write(path, data)
                    .await
                    .map_err(|e| anyhow!("Failed to write file: {e}"))
            }
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

        self.write_file(user).await?;

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
        let url = format!("/static/{}", &relative_path);
        self.write_file(user).await?;
        match Media::new(
            self.key.as_ref().unwrap().clone(),
            user.id,
            relative_path,
            self.title.clone(),
            hash,
            self.size() as i64,
        )
        .create(pool)
        .await
        {
            Ok(()) => Ok(UploadResponse::new(
                Some(self.key.as_ref().unwrap().clone()),
                url,
            )),
            Err(e) => {
                // Remove file as part of cleanup.
                tokio::fs::remove_file(&self.real_path(user)?).await.ok();
                Err(anyhow!("Database error: {e}"))
            }
        }
    }

    async fn save(&mut self, pool: &SqlitePool, user: &User) -> anyhow::Result<UploadResponse> {
        match self.role {
            ImageRole::Media => self.save_media(pool, user).await,
            ImageRole::Thumbnail(..) => self.save_thumbnail(pool, user).await,
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
        match sqlx::Row::try_get(&row, 0) {
            Ok(s) => Ok(Some(s)),
            Err(_) => Err(anyhow::anyhow!("Database error.")),
        }
    } else {
        Ok(None)
    }
}

async fn collect_part(part: Field) -> anyhow::Result<Vec<u8>> {
    part.try_fold(Vec::new(), |mut vec, data| {
        bytes::BufMut::put(&mut vec, data);
        async move { Ok(vec) }
    })
    .await
    .map_err(|e| anyhow!("Failed to read part: {e}"))
}

fn ext_from_filename(filename: &str) -> Option<String> {
    let mut ext = String::new();
    for c in filename.chars().rev() {
        match c {
            '.' => return Some(ext),
            _ => ext.push(c),
        }
    }
    None
}

fn choose_file_extension(part: &Field) -> Option<String> {
    if let Some((t, st)) = part.content_type().map(|m| (m.type_(), m.subtype())) {
        match (t, st) {
            (mime::IMAGE, mime::JPEG) => return Some("jpg".to_string()),
            (mime::IMAGE, mime::PNG) => return Some("png".to_string()),
            _ => {}
        }
    };

    part.content_disposition()
        .get_filename()
        .and_then(ext_from_filename)
}

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

async fn upload(pool: web::Data<SqlitePool>, user: User, mut form: Multipart) -> Res {
    let total_uploaded = Media::user_total_size(&pool, user.id).await.map_err(e500)?;

    // If they're already full, don't bother processing the upload.
    if total_uploaded >= UPLOAD_LIMIT {
        return res_failure("Upload limit exceeded.");
    }

    let mut upload = UploadImage::new().map_err(e500)?;

    while let Some(Ok(part)) = form.next().await {
        match part.name() {
            "thumbnail" => match collect_part(part).await.map(String::from_utf8) {
                Ok(Ok(scene_key)) => upload.role = ImageRole::Thumbnail(scene_key),
                _ => return res_failure("Bad thumbnail scene ID."),
            },
            "image" => {
                if let Some(ext) = choose_file_extension(&part) {
                    upload.ext = ext;
                } else {
                    return res_failure("Missing file type.");
                }

                match part.content_disposition().get_filename() {
                    Some(s) => upload.title = s.to_owned(),
                    None => upload.title = format!("untitled.{}", upload.ext),
                };

                upload.data = Some(collect_part(part).await.map_err(e500)?);
            }
            _ => (),
        }
    }

    if total_uploaded + upload.size() >= UPLOAD_LIMIT {
        return res_failure("Upload limit exceeded.");
    }

    let res = upload.save(&pool, &user).await.map_err(e500)?;
    res_json(res)
}
