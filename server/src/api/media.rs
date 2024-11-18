use actix_web::{web, HttpResponse};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::{res_failure, res_success, Resp};
use crate::{
    fs::{join_relative_path, CONTENT},
    models::{Media, User},
    req::e500,
    utils::format_uuid,
};

pub fn routes() -> actix_web::Scope {
    web::scope("/media")
        .route("/list", web::get().to(list))
        .route("/details", web::post().to(update))
        .route("/{media_uuid}", web::get().to(retrieve))
        .route("/{media_uuid}", web::delete().to(delete))
}

#[cfg_attr(test, derive(serde_derive::Deserialize))]
#[derive(serde_derive::Serialize)]
struct MediaItem {
    uuid: String,
    title: String,
    url: String,
    w: f32,
    h: f32,
}

impl MediaItem {
    fn from(record: Media) -> Self {
        Self {
            uuid: format_uuid(record.uuid),
            title: record.title,
            url: record.relative_path,
            w: record.w,
            h: record.h,
        }
    }
}

#[cfg_attr(test, derive(serde_derive::Deserialize))]
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

#[cfg_attr(test, derive(serde_derive::Serialize))]
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

#[cfg_attr(test, derive(serde_derive::Deserialize))]
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

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use actix_web::{test, web::Data, App};

    use super::{DetailsUpdate, MediaItemResponse, MediaListResponse};
    use crate::{
        api::{routes, Binary},
        fs::initialise_database,
        models::{Media, User},
        utils::format_uuid,
    };

    #[actix_web::test]
    async fn test_media_api() {
        // TEST
        //   GET /api/media/list
        //   POST /api/media/details
        //   GET /api/media/{uuid}
        //   DELETE /api/media/{uuid}

        let db = initialise_database().await.unwrap();
        let app =
            test::init_service(App::new().app_data(Data::new(db.clone())).service(routes())).await;
        let conn = &mut db.acquire().await.unwrap();

        // Create a couple of media items.
        let user = User::generate(conn).await;
        let r1 = Media::prepare(&user, "png", "image", "FILE_HASH", 1)
            .create(conn)
            .await
            .unwrap();
        let r2 = Media::prepare(&user, "jpg", "image", "FILE_2_HASH", 2)
            .create(conn)
            .await
            .unwrap();
        let mut items = HashSet::new();
        items.insert(format_uuid(r1.uuid));
        items.insert(format_uuid(r2.uuid));

        // Check that the list endpoint returns a list with those items.
        let session = user.session(conn).await;
        let req = test::TestRequest::get()
            .uri("/api/media/list")
            .cookie(session.clone())
            .to_request();
        let resp: MediaListResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);
        assert_eq!(resp.items.len(), 2);
        assert_eq!(
            resp.items
                .into_iter()
                .map(|i| i.uuid)
                .collect::<HashSet<String>>(),
            items
        );

        // Delete the second item.
        let req = test::TestRequest::delete()
            .uri(&format!("/api/media/{}", format_uuid(r2.uuid)))
            .cookie(session.clone())
            .to_request();
        let resp: Binary = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);

        // Check that the item no longer appears in the list.
        let req = test::TestRequest::get()
            .uri("/api/media/list")
            .cookie(session.clone())
            .to_request();
        let resp: MediaListResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);
        assert_eq!(resp.items.len(), 1);
        let item = resp.items.first().unwrap();
        assert_eq!(item.uuid, format_uuid(r1.uuid));

        // Update details of the first item.
        let req = test::TestRequest::post()
            .uri("/api/media/details")
            .cookie(session.clone())
            .set_json(DetailsUpdate {
                uuid: r1.uuid,
                title: "New Title!".to_string(),
                w: 5.,
                h: 8.,
            })
            .to_request();
        let resp: Binary = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);

        // Check that the details are updated.
        let req = test::TestRequest::get()
            .uri(&format!("/api/media/{}", format_uuid(r1.uuid)))
            .cookie(session.clone())
            .to_request();
        let resp: MediaItemResponse = test::call_and_read_body_json(&app, req).await;
        assert!(resp.success);
        let item = resp.details;
        assert_eq!(item.uuid, format_uuid(r1.uuid));
        assert_eq!(item.title, "New Title!".to_string());
        assert_eq!(item.w, 5.);
        assert_eq!(item.h, 8.);
    }
}
