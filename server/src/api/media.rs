use actix_web::{web, HttpResponse};
use sqlx::SqlitePool;

use super::{res_failure, res_success, Res};
use crate::{
    models::{Media, User},
    req::e500,
    utils::join_relative_path,
};

pub fn routes() -> actix_web::Scope {
    web::scope("/media")
        .route("/list", web::get().to(list))
        .route("/details", web::post().to(update))
        .route("/{media_key}", web::get().to(retrieve))
        .route("/{media_key}", web::delete().to(delete))
}

#[derive(serde_derive::Serialize, sqlx::FromRow)]
struct MediaItem {
    media_key: String,
    title: String,

    #[sqlx(rename = "relative_path")]
    url: String,
    w: f32,
    h: f32,
}

impl MediaItem {
    fn from(record: Media) -> Self {
        Self {
            media_key: record.media_key,
            title: record.title,
            url: record.relative_path,
            w: record.w,
            h: record.h,
        }
    }
}

#[derive(serde_derive::Serialize)]
struct MediaListResponse {
    items: Vec<MediaItem>,
    success: bool,
}

impl MediaListResponse {
    fn new(items: Vec<MediaItem>) -> MediaListResponse {
        MediaListResponse {
            items,
            success: true,
        }
    }
}

async fn list(pool: web::Data<SqlitePool>, user: User) -> Res {
    let media = Media::user_media(&pool, user.id).await.map_err(e500)?;
    let items = media.into_iter().map(MediaItem::from).collect();
    Ok(HttpResponse::Ok().json(&MediaListResponse::new(items)))
}

#[derive(serde_derive::Deserialize)]
struct DetailsUpdate {
    media_key: String,
    title: String,
    w: f32,
    h: f32,
}

async fn update(pool: web::Data<SqlitePool>, user: User, details: web::Json<DetailsUpdate>) -> Res {
    Media::update(
        &pool,
        user.id,
        details.media_key.clone(),
        details.title.clone(),
        details.w,
        details.h,
    )
    .await
    .map_err(e500)?;

    res_success("Media updated.")
}

#[derive(serde_derive::Serialize)]
struct MediaItemResponse {
    details: MediaItem,
    success: bool,
}

impl MediaItemResponse {
    fn new(item: MediaItem) -> Self {
        Self {
            details: item,
            success: true,
        }
    }
}

async fn retrieve(
    pool: web::Data<SqlitePool>,
    path: web::Path<(String,)>,
) -> impl actix_web::Responder {
    let media_key = path.into_inner().0;
    let media = match Media::load(&pool, &media_key).await {
        Ok(record) => record,
        _ => return res_failure("Media not found."),
    };

    Ok(HttpResponse::Ok().json(&MediaItemResponse::new(MediaItem::from(media))))
}

async fn delete(pool: web::Data<SqlitePool>, user: User, path: web::Path<(String,)>) -> Res {
    let media_key = path.into_inner().0;
    if let Ok(media) = Media::load(&pool, &media_key).await {
        if user.id == media.user && Media::delete(&pool, &media_key).await.is_ok() {
            tokio::fs::remove_file(join_relative_path(
                crate::CONTENT.as_path(),
                media.relative_path,
            ))
            .await
            .ok();
            return res_success("Media deleted successfully.");
        }
    }

    res_failure("Media not found.")
}
