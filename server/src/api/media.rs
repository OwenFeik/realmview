use actix_web::{web, HttpResponse};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::{res_failure, res_success, Resp};
use crate::{
    fs::{join_relative_path, CONTENT},
    models::{Media, User},
    req::e500,
};

pub fn routes() -> actix_web::Scope {
    web::scope("/media")
        .route("/list", web::get().to(list))
        .route("/details", web::post().to(update))
        .route("/{media_uuid}", web::get().to(retrieve))
        .route("/{media_uuid}", web::delete().to(delete))
}

#[derive(serde_derive::Serialize)]
struct MediaItem {
    uuid: Uuid,
    title: String,
    url: String,
    w: f32,
    h: f32,
}

impl MediaItem {
    fn from(record: Media) -> Self {
        Self {
            uuid: record.uuid,
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

async fn list(pool: web::Data<SqlitePool>, user: User) -> Resp {
    let conn = &mut pool.acquire().await.map_err(e500)?;
    let media = Media::user_media(conn, user.uuid).await.map_err(e500)?;
    let items = media.into_iter().map(MediaItem::from).collect();
    Ok(HttpResponse::Ok().json(MediaListResponse::new(items)))
}

#[derive(serde_derive::Deserialize)]
struct DetailsUpdate {
    uuid: Uuid,
    title: String,
    w: f32,
    h: f32,
}

async fn update(
    pool: web::Data<SqlitePool>,
    user: User,
    details: web::Json<DetailsUpdate>,
) -> Resp {
    let conn = &mut pool.acquire().await.map_err(e500)?;
    Media::update(
        conn,
        user.uuid,
        details.uuid,
        &details.title,
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
    let conn = &mut pool.acquire().await.map_err(e500)?;
    let uuid = match Uuid::try_parse(&path.into_inner().0) {
        Ok(uuid) => uuid,
        _ => return res_failure("Invalid media UUID."),
    };
    let media = match Media::load(conn, uuid).await {
        Ok(record) => record,
        _ => return res_failure("Media not found."),
    };

    Ok(HttpResponse::Ok().json(MediaItemResponse::new(MediaItem::from(media))))
}

async fn delete(pool: web::Data<SqlitePool>, user: User, path: web::Path<(String,)>) -> Resp {
    let conn = &mut pool.acquire().await.map_err(e500)?;
    let uuid = match Uuid::try_parse(&path.into_inner().0) {
        Ok(uuid) => uuid,
        _ => return res_failure("Invalid media UUID."),
    };
    if let Ok(media) = Media::load(conn, uuid).await {
        if user.uuid == media.user && Media::delete(conn, uuid).await.is_ok() {
            tokio::fs::remove_file(join_relative_path(&CONTENT, media.relative_path))
                .await
                .ok();
            return res_success("Media deleted successfully.");
        }
    }

    res_failure("Media not found.")
}
