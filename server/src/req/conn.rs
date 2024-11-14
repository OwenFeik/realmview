use std::pin::Pin;

use actix_web::FromRequest;
use futures::Future;
use sqlx::{pool::PoolConnection, Sqlite, SqliteConnection};

use super::e500;
use crate::fs::initialise_database;

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
            Ok(Pool(
                initialise_database()
                    .await
                    .map_err(e500)?
                    .acquire()
                    .await
                    .map_err(e500)?,
            ))
        })
    }
}
