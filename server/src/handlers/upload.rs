use std::convert::Infallible;

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
    media_key: String,
    url: String,
}

impl UploadResponse {
    fn new(key: String, url: String) -> UploadResponse {
        UploadResponse {
            message: String::from("Uploaded successfully."),
            success: true,
            media_key: key,
            url,
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

    for p in parts {
        if p.name() != "image" {
            continue;
        }

        let ext = match p.content_type() {
            Some(mime) => match mime {
                "image/png" => "png",
                "image/jpeg" => "jpeg",
                _ => {
                    return Binary::result_failure(
                        format!("Unsupported image type: {}", mime).as_str(),
                    )
                }
            },
            None => return Binary::result_failure("Missing content type."),
        };

        let title = match p.filename() {
            Some(s) => s.to_string(),
            None => format!("untitled.{}", ext),
        };

        let data = p
            .stream()
            .try_fold(Vec::new(), |mut vec, data| {
                vec.put(data);
                async move { Ok(vec) }
            })
            .await
            .map_err(|_| anyhow::anyhow!("Failed to read file"));

        let data = match data {
            Ok(v) => v,
            Err(_) => return Binary::result_failure("Failed to read file."),
        };

        let hash = match hash_file(&data) {
            Ok(h) => h,
            Err(_) => return Binary::result_error("Failed to hash file."),
        };

        match file_exists(&pool, user.id, &hash).await {
            Err(_) => return Binary::result_error("Database error checking for duplicate."),
            Ok(Some(existing_title)) => {
                return {
                    if existing_title == title {
                        Binary::result_failure("File already uploaded.")
                    } else {
                        Binary::result_failure(&format!(
                            "File already uploaded as {}",
                            &existing_title
                        ))
                    }
                }
            }
            _ => (),
        };

        let key = match Media::generate_key() {
            Ok(s) => s,
            Err(_) => return Binary::result_error("File name generation failed."),
        };

        let relative_path = format!("{}/{}.{}", &user.relative_dir(), key, ext);
        if tokio::fs::create_dir_all(user.upload_dir(&content_dir))
            .await
            .is_err()
        {
            return Binary::result_error("Failed to create upload dir.");
        }

        let real_path = format!("{}/{}", content_dir, &relative_path);
        if tokio::fs::write(&real_path, data).await.is_err() {
            return Binary::result_error("Failed to write file.");
        };

        return match Media::create(&pool, &key, user.id, &relative_path, &title, &hash).await {
            Ok(media) => {
                let url = format!("/static/{}", &relative_path);
                as_result(
                    &UploadResponse::new(media.media_key, url),
                    warp::http::StatusCode::OK,
                )
            }
            Err(_) => {
                // Remove file as part of cleanup.
                tokio::fs::remove_file(&real_path).await.ok();
                Binary::result_error("Database error.")
            }
        };
    }

    Binary::result_failure("No image provided.")
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
