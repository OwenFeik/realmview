use core::convert::Infallible;
use warp::Filter;

use sqlx::SqlitePool;

use super::{session_user, with_db, with_session};
use super::response::{Binary, as_result};

#[derive(serde_derive::Serialize, sqlx::FromRow)]
struct MediaItem {
    id: i64,
    title: String,

    #[sqlx(rename = "relative_path")]
    url: String
}

#[derive(serde_derive::Serialize)]
struct MediaResponse {
    items: Vec<MediaItem>,
    success: bool
}

impl MediaResponse {
    fn new(items: Vec<MediaItem>) -> MediaResponse {
        MediaResponse { items, success: true }
    }
}

async fn user_media(pool: &SqlitePool, user_id: i64) -> anyhow::Result<Vec<MediaItem>> {
    let results = sqlx::query_as("SELECT id, title, relative_path FROM media WHERE user = ?1;")
        .bind(user_id)
        .fetch_all(pool)
        .await?;
    Ok(results)
}

async fn media(pool: SqlitePool, session_key: Option<String>) -> Result<impl warp::Reply, Infallible> {
    let user = match session_user(&pool, session_key).await {
        Ok(u) => u,
        Err(r) => return r
    };

    match user_media(&pool, user.id).await {
        Ok(media) => as_result(&MediaResponse::new(media), warp::http::StatusCode::OK),
        Err(_) => Binary::result_error("Database error.")
    }
}

pub fn filter(pool: SqlitePool) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("media")
        .and(warp::get())
        .and(with_db(pool))
        .and(with_session())
        .and_then(media)
}
