use warp::Filter;

use sqlx::SqlitePool;

#[derive(serde_derive::Serialize, sqlx::FromRow)]
struct MediaItem {
    media_key: String,
    title: String,

    #[sqlx(rename = "relative_path")]
    url: String,
}

mod details {
    use warp::Filter;

    use sqlx::SqlitePool;

    use crate::handlers::response::{as_result, Binary, ResultReply};
    use crate::handlers::{json_body, with_db, with_session, with_string};
    use crate::models::{Media, User};

    #[derive(serde_derive::Serialize)]
    struct MediaItemResponse {
        details: super::MediaItem,
        success: bool,
    }

    impl MediaItemResponse {
        fn new(item: super::MediaItem) -> Self {
            Self {
                details: item,
                success: true,
            }
        }

        fn result(item: super::MediaItem) -> ResultReply {
            as_result(&MediaItemResponse::new(item), warp::hyper::StatusCode::OK)
        }
    }

    #[derive(serde_derive::Deserialize)]
    struct DetailsUpdate {
        media_key: String,
        title: String,
    }

    async fn update_in_db(
        pool: &SqlitePool,
        user_id: i64,
        key: String,
        title: String,
    ) -> anyhow::Result<()> {
        sqlx::query("UPDATE media SET title = ?1 WHERE media_key = ?2 AND user = ?3;")
            .bind(&title)
            .bind(key)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn update_media(pool: SqlitePool, skey: String, details: DetailsUpdate) -> ResultReply {
        let user = match User::get_by_session(&pool, &skey).await {
            Ok(Some(user)) => user,
            _ => return Binary::result_failure("Invalid session."),
        };

        match update_in_db(&pool, user.id, details.media_key, details.title).await {
            Ok(()) => Binary::result_success("Media updated."),
            Err(_) => Binary::result_error("Database error."),
        }
    }

    fn update_filter(
        pool: SqlitePool,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("media" / "details")
            .and(warp::post())
            .and(with_db(pool))
            .and(with_session())
            .and(json_body::<DetailsUpdate>())
            .and_then(update_media)
    }

    async fn media_details(key: String, pool: SqlitePool) -> ResultReply {
        let record = match Media::load(&pool, &key).await {
            Ok(record) => record,
            _ => return Binary::result_failure("Media not found."),
        };

        MediaItemResponse::result(super::MediaItem {
            media_key: record.media_key,
            title: record.title,
            url: record.relative_path,
        })
    }

    fn retrieve_filter(
        pool: SqlitePool,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("media" / String)
            .and(warp::get())
            .and(with_db(pool))
            .and_then(media_details)
    }

    async fn media_delete(
        key: String,
        skey: String,
        pool: SqlitePool,
        content_dir: String,
    ) -> ResultReply {
        let user = match User::get_by_session(&pool, &skey).await {
            Ok(Some(user)) => user,
            _ => return Binary::result_failure("Invalid session."),
        };

        let media = match Media::load(&pool, &key).await {
            Ok(record) => record,
            _ => return Binary::result_failure("Media not found."),
        };

        if user.id == media.user {
            if Media::delete(&pool, &key).await.is_ok() {
                tokio::fs::remove_file(format!("{}/{}", content_dir, media.relative_path))
                    .await
                    .ok();
            } else {
                return Binary::result_failure("Media not found.");
            }
        }

        Binary::result_success("Media deleted successfully.")
    }

    fn delete_filter(
        pool: SqlitePool,
        content_dir: String,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("media" / String)
            .and(warp::delete())
            .and(with_session())
            .and(with_db(pool))
            .and(with_string(content_dir))
            .and_then(media_delete)
    }

    pub fn filter(
        pool: SqlitePool,
        content_dir: String,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        update_filter(pool.clone())
            .or(retrieve_filter(pool.clone()))
            .or(delete_filter(pool, content_dir))
    }
}

mod list {
    use core::convert::Infallible;
    use sqlx::SqlitePool;
    use warp::Filter;

    use crate::handlers::{
        response::{as_result, Binary},
        with_db, with_session,
    };
    use crate::models::User;

    use super::MediaItem;

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

    async fn user_media(pool: &SqlitePool, user_id: i64) -> anyhow::Result<Vec<MediaItem>> {
        let results =
            sqlx::query_as("SELECT media_key, title, relative_path FROM media WHERE user = ?1;")
                .bind(user_id)
                .fetch_all(pool)
                .await?;
        Ok(results)
    }

    async fn list_media(pool: SqlitePool, skey: String) -> Result<impl warp::Reply, Infallible> {
        let user = match User::get_by_session(&pool, &skey).await {
            Ok(Some(user)) => user,
            _ => return Binary::result_failure("Invalid session."),
        };

        match user_media(&pool, user.id).await {
            Ok(media) => as_result(&MediaListResponse::new(media), warp::http::StatusCode::OK),
            Err(_) => Binary::result_error("Database error."),
        }
    }

    pub fn filter(
        pool: SqlitePool,
    ) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path("media")
            .and(warp::path("list"))
            .and(with_db(pool))
            .and(with_session())
            .and_then(list_media)
    }
}

pub fn filter(
    pool: SqlitePool,
    content_dir: String,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    list::filter(pool.clone()).or(details::filter(pool, content_dir))
}
