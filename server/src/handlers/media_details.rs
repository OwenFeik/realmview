use core::convert::Infallible;
use warp::Filter;

use sqlx::SqlitePool;

use super::response::Binary;
use super::{json_body, session_user, with_db, with_session};

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
    session_key: Option<String>,
    details: DetailsUpdate,
) -> Result<impl warp::Reply, Infallible> {
    let user = match session_user(&pool, session_key).await {
        Ok(u) => u,
        Err(r) => return r,
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
    warp::path!("media_details")
        .and(warp::post())
        .and(with_db(pool))
        .and(with_session())
        .and(json_body::<DetailsUpdate>())
        .and_then(update_media)
}
