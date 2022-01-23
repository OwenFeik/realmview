use bytes::BufMut;
use futures::TryStreamExt;
use std::convert::Infallible;

use sqlx::SqlitePool;
use warp::{Filter, multipart::{FormData, Part}};

use super::crypto::random_hex_string;
use super::models::UserSession;
use super::response::Binary;
use super::{with_db, with_session};


async fn upload(pool: SqlitePool, session_key: Option<String>, content_dir: String, form: FormData)
    -> Result<impl warp::Reply, Infallible>
{
    let upload_dir = match session_key {
        Some(k) => match UserSession::get(&pool, k.as_str()).await {
            Ok(Some(session)) => match session.user(&pool).await {
                Ok(Some(user)) => user.upload_dir(content_dir.as_str()),
                Ok(None) => return Binary::result_failure("User not found."),
                Err(_) => return Binary::result_error("Database error.")
            },
            Ok(None) => return Binary::result_failure("Session not found."),
            Err(_) => return Binary::result_error("Database error.")
        },
        None => return Binary::result_failure("Session required.")
    };
    
    let parts: Vec<Part> = match form.try_collect().await {
        Ok(v) => v,
        Err(_) => return Binary::result_failure("Upload failed.")
    };
    
    for p in parts {
        if p.name() != "image" {
            continue;
        }

        let ext = match p.content_type() {
            Some(mime) => match mime {
                "image/png" => "png",
                "image/jpeg" => "jpeg",
                _ => return Binary::result_failure(format!("Unsupported image type: {}", mime).as_str())
            },
            None => return Binary::result_failure("Missing content type.")
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

        let file_path = match random_hex_string(16) {
            Ok(s) => format!("{}/{}.{}", upload_dir.as_str(), s.as_str(), ext),
            Err(_) => return Binary::result_error("File name generation failed.")
        };

        match tokio::fs::write(&file_path, data).await {
            Err(_) => return Binary::result_error("Failed to write file."),
            _ => ()
        };
    }

    Binary::result_success("Uploaded successfully.")
}

fn with_string(string: String) -> impl Filter<Extract = (String,), Error = Infallible> + Clone {
    warp::any().map(move || string.clone())
}

pub fn filter(pool: SqlitePool, content_dir: String) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("upload")
        .and(warp::post())
        .and(with_db(pool))
        .and(with_session())
        .and(with_string(content_dir))
        .and(warp::multipart::form())
        .and_then(upload)
}
