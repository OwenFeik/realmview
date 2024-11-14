use actix_multipart::{Field, Multipart};
use actix_web::{error::ErrorInternalServerError, web};
use futures::{StreamExt, TryStreamExt};
use ring::digest;
use sqlx::{SqliteConnection, SqlitePool};
use uuid::Uuid;

use super::{res_failure, res_json, Resp};
use crate::{
    crypto::format_hex,
    fs::{join_relative_path, write_file, CONTENT},
    models::{Media, User},
    req::e500,
    utils::{err, format_uuid, Res},
};

// Maximum total size of media a single use can upload, in bytes
const UPLOAD_LIMIT: usize = 10 * 1024 * 1024 * 1024; // 10 GB

pub fn routes() -> actix_web::Scope {
    web::scope("/upload").default_service(web::route().to(upload))
}

#[derive(Debug)]
enum ImageRole {
    Media,
    Thumbnail(Uuid),
}

struct UploadImage {
    role: ImageRole,
    data: Option<Vec<u8>>,
    title: String,
    ext: String,
}

impl UploadImage {
    const DEFAULT_TITLE: &'static str = "untitled";
    const DEFAULT_EXT: &'static str = "png";

    fn new() -> Res<Self> {
        Ok(UploadImage {
            role: ImageRole::Media,
            data: None,
            title: Self::DEFAULT_TITLE.to_owned(),
            ext: Self::DEFAULT_EXT.to_owned(),
        })
    }

    fn size(&self) -> usize {
        match &self.data {
            Some(data) => data.len(),
            None => 0,
        }
    }

    async fn submit(self, pool: &SqlitePool, user: &User) -> Res<UploadResponse> {
        let conn = &mut pool
            .acquire()
            .await
            .map_err(|e| format!("Failed to acquire database connection: {e} "))?;

        match self.data {
            Some(data) => match self.role {
                ImageRole::Media => save_media(conn, user, data, self.title, self.ext).await,
                ImageRole::Thumbnail(scene) => save_thumbnail(conn, user, data, scene).await,
            },
            None => err("No image data provided."),
        }
    }
}

async fn save_media(
    conn: &mut SqliteConnection,
    user: &User,
    data: Vec<u8>,
    title: String,
    ext: String,
) -> Res<UploadResponse> {
    let hash = hash_file(&data);

    if let Some(existing_title) = Media::exists(conn, user.uuid, &hash).await? {
        return {
            if existing_title == title {
                err("File already uploaded.")
            } else {
                Err(format!("File already uploaded as {}", &existing_title))
            }
        };
    }

    let record = Media::prepare(
        user.uuid,
        &format!("/uploads/{}", &user.username),
        &ext,
        title,
        hash,
        data.len() as i64,
    );

    let path = join_relative_path(&CONTENT, &record.relative_path);
    write_file(&path, &data).await?;

    let url = format!("/static/{}", &record.relative_path);
    match record.create(conn).await {
        Ok(record) => Ok(UploadResponse::new(Some(format_uuid(record.uuid)), url)),
        Err(e) => {
            // Remove file as part of cleanup.
            tokio::fs::remove_file(&path).await.ok();
            Err(format!("Database error: {e}"))
        }
    }
}

async fn save_thumbnail(
    conn: &mut SqliteConnection,
    user: &User,
    data: Vec<u8>,
    scene: Uuid,
) -> Res<UploadResponse> {
    if crate::models::Project::for_scene(conn, scene).await?.user != user.uuid {
        return err("User does not own scene.");
    }

    let relative_path = format!(
        "/uploads/{}/thumbnails/{}.png",
        &user.username,
        format_uuid(scene),
    );
    let absolute_path = join_relative_path(&CONTENT, &relative_path);
    write_file(&absolute_path, &data).await?;

    crate::models::Scene::set_thumbnail(conn, scene, &relative_path).await?;

    Ok(UploadResponse::new(
        None,
        format!("/static/{}", &relative_path),
    ))
}

fn hash_file(raw: &[u8]) -> String {
    format_hex(digest::digest(&digest::SHA256, raw).as_ref())
}

async fn collect_part(part: Field) -> Res<Vec<u8>> {
    part.try_fold(Vec::new(), |mut vec, data| {
        bytes::BufMut::put(&mut vec, data);
        async move { Ok(vec) }
    })
    .await
    .map_err(|e| format!("Failed to read part: {e}"))
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
    uuid: Option<String>,
    url: String,
}

impl UploadResponse {
    fn new(uuid: Option<String>, url: String) -> UploadResponse {
        UploadResponse {
            message: String::from("Uploaded successfully."),
            success: true,
            uuid,
            url,
        }
    }
}

async fn upload(pool: web::Data<SqlitePool>, user: User, mut form: Multipart) -> Resp {
    let conn = &mut pool.acquire().await.map_err(ErrorInternalServerError)?;
    let total_uploaded = Media::user_total_size(conn, user.uuid)
        .await
        .map_err(e500)?;

    // If they're already full, don't bother processing the upload.
    if total_uploaded >= UPLOAD_LIMIT {
        return res_failure("Upload limit exceeded.");
    }

    let mut upload = UploadImage::new().map_err(e500)?;

    while let Some(Ok(part)) = form.next().await {
        match part.name() {
            "thumbnail" => match collect_part(part)
                .await
                .and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()))
                .and_then(|s| Uuid::try_parse(&s).map_err(|e| e.to_string()))
            {
                Ok(uuid) => upload.role = ImageRole::Thumbnail(uuid),
                _ => return res_failure("Bad thumbnail scene ID."),
            },
            "image" => {
                if let Some(ext) = choose_file_extension(&part) {
                    upload.ext = ext;
                } else {
                    return res_failure("Missing file type.");
                }

                match part.content_disposition().get_filename() {
                    Some(s) => s.clone_into(&mut upload.title),
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

    let res = upload.submit(&pool, &user).await.map_err(e500)?;
    res_json(res)
}
