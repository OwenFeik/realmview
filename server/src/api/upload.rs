use actix_multipart::{Field, Multipart};
use actix_web::{error::ErrorInternalServerError, web};
use futures::{StreamExt, TryStreamExt};
use ring::digest;
use sqlx::{SqliteConnection, SqlitePool};
use uuid::Uuid;

use super::{res_failure, res_json, Resp};
use crate::{
    crypto::to_hex_string,
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

    let record = Media::prepare(user, &ext, title, hash, data.len());

    let path = join_relative_path(&CONTENT, &record.relative_path);
    write_file(&path, &data).await?;

    match record.create(conn).await {
        Ok(record) => Ok(UploadResponse::new(
            Some(format_uuid(record.uuid)),
            record.relative_path,
        )),
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

    Ok(UploadResponse::new(None, relative_path))
}

fn hash_file(raw: &[u8]) -> String {
    to_hex_string(digest::digest(&digest::SHA256, raw).as_ref())
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

#[cfg_attr(test, derive(serde_derive::Deserialize))]
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

#[cfg(test)]
mod test {
    use std::io::Write;

    use actix_web::{
        http::header::{HeaderName, HeaderValue},
        test,
        web::Data,
        App,
    };

    use super::UploadResponse;
    use crate::{
        fs::{initialise_database, join_relative_path, CONTENT},
        models::{Media, Project, Scene, User},
        utils::{format_uuid, generate_uuid, parse_uuid},
    };

    fn multipart_request(
        filename: &str,
        image_data: &[u8],
        extra: Option<(&str, &str)>,
    ) -> (Vec<u8>, (HeaderName, HeaderValue)) {
        let boundary = format_uuid(generate_uuid());
        let mut body = Vec::new();

        let ct = if filename.ends_with("jpg") || filename.ends_with("jpeg") {
            "jpeg"
        } else {
            "png"
        };

        if let Some((key, value)) = extra {
            write!(&mut body, "--{}\r\n", &boundary).unwrap();
            write!(&mut body, "Content-Disposition: form-data; ").unwrap();
            write!(&mut body, "name=\"{}\"\r\n\r\n", key).unwrap();
            write!(&mut body, "{}\r\n", value).unwrap();
        }

        write!(&mut body, "--{}\r\n", &boundary).unwrap();
        write!(&mut body, "Content-Disposition: form-data; ").unwrap();
        write!(&mut body, "name=\"image\"; filename=\"{}\"\r\n", filename).unwrap();
        write!(&mut body, "Content-Type: image/{}\r\n\r\n", ct).unwrap();
        body.write_all(image_data).unwrap();
        write!(&mut body, "\r\n--{}--", &boundary).unwrap();

        let header = (
            actix_web::http::header::CONTENT_TYPE,
            HeaderValue::from_str(&format!("multipart/form-data; boundary={}", &boundary)).unwrap(),
        );

        (body, header)
    }

    #[actix_web::test]
    async fn test_upload_media() {
        let db = initialise_database().await.unwrap();
        let app = test::init_service(
            App::new()
                .app_data(Data::new(db.clone()))
                .service(crate::api::routes())
                .service(crate::content::routes()),
        )
        .await;

        let conn = &mut db.acquire().await.unwrap();

        // Create request payload and headers.
        let image_data: Vec<u8> = (0..=255).collect();
        let (payload, header) = multipart_request("image.jpg", &image_data, None);

        // Upload should fail without authentication, redirecting to login.
        let req = test::TestRequest::post()
            .uri("/api/upload")
            .append_header(header.clone())
            .set_payload(payload.clone())
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_redirection());

        // Upload with authentication.
        let user = User::generate(conn).await;
        let session = user.session(conn).await;
        let req = test::TestRequest::post()
            .uri("/api/upload")
            .cookie(session.clone())
            .append_header(header)
            .set_payload(payload)
            .to_request();
        let resp: UploadResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);
        assert!(resp.uuid.is_some());

        // Validate the record in the database has the appropriate details.
        let record = Media::load(conn, parse_uuid(&resp.uuid.unwrap()).unwrap())
            .await
            .unwrap();
        assert_eq!(record.file_size, image_data.len());
        assert_eq!(record.user, user.uuid);
        assert!(
            tokio::fs::try_exists(join_relative_path(&CONTENT, &resp.url))
                .await
                .unwrap()
        );

        // Test that we can request the uploaded image.
        let req = test::TestRequest::get().uri(&resp.url).to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.headers().get("Content-Type").unwrap(), "image/jpeg");
        assert_eq!(test::read_body(resp).await, image_data);
    }

    #[actix_web::test]
    async fn test_upload_thumbnail() {
        let db = initialise_database().await.unwrap();
        let app = test::init_service(
            App::new()
                .app_data(Data::new(db.clone()))
                .service(crate::api::routes())
                .service(crate::content::routes()),
        )
        .await;
        let conn = &mut db.acquire().await.unwrap();

        // Create a scene to upload a thumbnail for.
        let user = User::generate(conn).await;
        let project = Project::create(conn, &user, "My Project").await.unwrap();
        let mut proj = project.load(conn).await.unwrap();
        proj.new_scene();
        let (_, scenes) = Project::save(conn, &user, proj).await.unwrap();
        let uuid = scenes.first().unwrap().uuid;
        let record = Scene::get_by_uuid(conn, uuid).await.unwrap();
        assert!(record.thumbnail.is_none());

        // Create request payload and headers.
        let image_data: Vec<u8> = (0..=255).rev().cycle().take(1024).collect();
        let (payload, header) = multipart_request(
            "image.png",
            &image_data,
            Some(("thumbnail", &format_uuid(uuid))),
        );

        // Upload thumbnail.
        let session = user.session(conn).await;
        let req = test::TestRequest::post()
            .uri("/api/upload")
            .cookie(session.clone())
            .append_header(header)
            .set_payload(payload)
            .to_request();
        let resp: UploadResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);
        assert!(resp.uuid.is_none());
        let record = Scene::get_by_uuid(conn, uuid).await.unwrap();
        assert!(record.thumbnail.is_some());
        assert_eq!(&record.thumbnail.unwrap(), &resp.url);

        // Check that we can download the thumbnail.
        let req = test::TestRequest::get().uri(&resp.url).to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.headers().get("Content-Type").unwrap(), "image/png");
        assert_eq!(test::read_body(resp).await, image_data);
    }
}
