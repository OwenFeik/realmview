use std::pin::Pin;

use actix_web::{error::ErrorInternalServerError, FromRequest};
use futures::Future;
use sqlx::{pool::PoolConnection, Sqlite, SqliteConnection};

use crate::api::e500;

pub struct Conn(PoolConnection<Sqlite>);

impl Conn {
    pub fn acquire(&mut self) -> &mut SqliteConnection {
        &mut self.0
    }
}

impl FromRequest for Conn {
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Conn, Self::Error>>>>;

    fn from_request(
        _req: &actix_web::HttpRequest,
        _payload: &mut actix_web::dev::Payload,
    ) -> Self::Future {
        Box::pin(async {
            if let Some(Some(pool)) = super::POOL.get() {
                Ok(Conn(pool.acquire().await.map_err(e500)?))
            } else {
                Err(ErrorInternalServerError("Failed to acquire pool."))
            }
        })
    }
}
