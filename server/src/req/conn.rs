use std::pin::Pin;

use actix_web::{error::ErrorInternalServerError, FromRequest};
use futures::Future;
use sqlx::{pool::PoolConnection, Sqlite, SqliteConnection};

use super::e500;

pub struct Pool(PoolConnection<Sqlite>);

impl Pool {
    pub fn acquire(&mut self) -> &mut SqliteConnection {
        &mut self.0
    }
}

impl FromRequest for Pool {
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Pool, Self::Error>>>>;

    fn from_request(
        _req: &actix_web::HttpRequest,
        _payload: &mut actix_web::dev::Payload,
    ) -> Self::Future {
        Box::pin(async {
            if let Some(pool) = super::POOL.get() {
                Ok(Pool(pool.acquire().await.map_err(e500)?))
            } else {
                Err(ErrorInternalServerError("Failed to acquire pool."))
            }
        })
    }
}
