use std::convert::Infallible;

use bytes::BufMut;
use futures::TryStreamExt;
use ring::digest;
use sqlx::{Row, SqlitePool};
use warp::{
    multipart::{FormData, Part},
    Filter,
};

use super::crypto::{random_hex_string, to_hex_string_unsized};
use super::response::{as_result, Binary};
use super::{session_user, with_db, with_session};

#[derive(serde_derive::Serialize)]
struct UploadResponse {
    message: String,
    success: bool,
    id: i64,
    url: String,
}

impl UploadResponse {
    fn new(id: i64, url: String) -> UploadResponse {
        UploadResponse {
            message: String::from("Uploaded successfully."),
            success: true,
            id,
            url,
        }
    }
}

fn hash_file(raw: &[u8]) -> anyhow::Result<String> {
    to_hex_string_unsized(digest::digest(&digest::SHA256, raw).as_ref())
}

async fn file_exists(pool: &SqlitePool, hash: &str) -> anyhow::Result<Option<String>> {
    let row_opt = sqlx::query("SELECT title FROM media WHERE hashed_value = ?1;")
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
    session_key: Option<String>,
    content_dir: String,
    form: FormData,
) -> Result<impl warp::Reply, Infallible> {
    let user = match session_user(&pool, session_key).await {
        Ok(u) => u,
        Err(r) => return r,
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

        match file_exists(&pool, &hash).await {
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

        let relative_path = match random_hex_string(16) {
            Ok(s) => format!("{}/{}.{}", &user.relative_dir(), s.as_str(), ext),
            Err(_) => return Binary::result_error("File name generation failed."),
        };

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

        return match sqlx::query(
            "INSERT INTO media (user, relative_path, title, hashed_value) VALUES (?1, ?2, ?3, ?4) RETURNING id;"
        )
            .bind(user.id)
            .bind(&relative_path)
            .bind(title)
            .bind(hash)
            .fetch_one(&pool)
            .await
        {
            Ok(row) => match row.try_get(0) {
                Ok(id) => {
                    let url = format!("/static/{}", &relative_path);
                    as_result(&UploadResponse::new(id, url), warp::http::StatusCode::OK)
                },
                Err(_) => {
                    tokio::fs::remove_file(&real_path).await.ok();
                    Binary::result_error("Database error.")
                }
            },
            Err(_) => {
                // Remove file as part of cleanup.
                tokio::fs::remove_file(&real_path).await.ok();
                Binary::result_error("Database error.")
            }
        };
    }

    Binary::result_failure("No image provided.")
}

fn with_string(string: String) -> impl Filter<Extract = (String,), Error = Infallible> + Clone {
    warp::any().map(move || string.clone())
}

pub fn filter(
    pool: SqlitePool,
    content_dir: String,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("upload")
        .and(warp::post())
        .and(with_db(pool))
        .and(with_session())
        .and(with_string(content_dir))
        .and(warp::multipart::form())
        .and_then(upload)
}
