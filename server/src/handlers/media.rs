use warp::Filter;

use sqlx::SqlitePool;

#[derive(serde_derive::Serialize, sqlx::FromRow)]
struct MediaItem {
    id: i64,
    title: String,

    #[sqlx(rename = "relative_path")]
    url: String,
}

mod details {
    use core::convert::Infallible;
    use warp::Filter;

    use sqlx::SqlitePool;

    use crate::handlers::response::Binary;
    use crate::handlers::{json_body, with_db, with_session};
    use crate::models::User;

    #[derive(serde_derive::Deserialize)]
    struct DetailsUpdate {
        id: String,
        title: String,
    }

    async fn update_in_db(
        pool: &SqlitePool,
        user_id: i64,
        id: i64,
        title: String,
    ) -> anyhow::Result<()> {
        sqlx::query("UPDATE media SET title = ?1 WHERE id= ?2 AND user = ?3;")
            .bind(&title)
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn update_media(
        pool: SqlitePool,
        skey: String,
        details: DetailsUpdate,
    ) -> Result<impl warp::Reply, Infallible> {
        let user = match User::get_by_session(&pool, &skey).await {
            Ok(Some(user)) => user,
            _ => return Binary::result_failure("Invalid session."),
        };

        let id = match details.id.parse::<i64>() {
            Ok(id) => id,
            Err(_) => return Binary::result_failure("Non-integer ID provided."),
        };

        match update_in_db(&pool, user.id, id, details.title).await {
            Ok(()) => Binary::result_success("Media updated."),
            Err(_) => Binary::result_error("Database error."),
        }
    }

    pub fn filter(
        pool: SqlitePool,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path("media")
            .and(warp::path("details"))
            .and(warp::post())
            .and(with_db(pool))
            .and(with_session())
            .and(json_body::<DetailsUpdate>())
            .and_then(update_media)
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
        let results = sqlx::query_as("SELECT id, title, relative_path FROM media WHERE user = ?1;")
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
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path("media")
            .and(warp::path("list"))
            .and(with_db(pool))
            .and(with_session())
            .and_then(list_media)
    }
}

pub fn filter(
    pool: SqlitePool,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    list::filter(pool.clone()).or(details::filter(pool))
}
