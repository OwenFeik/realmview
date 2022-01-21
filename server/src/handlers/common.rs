use std::convert::Infallible;

use sqlx::sqlite::SqlitePool;
use warp::Filter;


pub mod response {
    use std::convert::Infallible;

    use serde::Serialize;
    use serde_derive::{Deserialize, Serialize};
    use warp::hyper::StatusCode;


    type JsonReply = Result<warp::reply::Json, Infallible>;

    #[derive(Deserialize, Serialize)]
    pub struct Binary {
        message: String,
        status: u16,
        success: bool
    }

    impl Binary {
        pub fn new(message: &str, success: bool) -> Binary {
            Binary { message: String::from(message), status: StatusCode::OK.as_u16(), success }
        }
    
        pub fn new_success(message: &str) -> Binary {
            Binary::new(message, true)
        }
    
        pub fn new_failure(message: &str) -> Binary {
            Binary::new(message, false)
        }
    
        pub fn reply_success(message: &str) -> JsonReply {
            as_reply(&Binary::new_success(message))
        }
    
        pub fn reply_failure(message: &str) -> JsonReply {
            as_reply(&Binary::new_failure(message))
        }

        pub fn reply_error(message: &str) -> JsonReply {
            as_reply(&Binary {
                message: String::from(message),
                status: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                success: false
            })
        }
    }

    pub fn as_reply(body: &impl Serialize) -> JsonReply {
        Ok(warp::reply::json(body))
    }
}


pub fn json_body<T: std::marker::Send + for<'de> serde::Deserialize<'de>>() ->
    impl Filter<Extract = (T,), Error = warp::Rejection> + Clone
{
    warp::body::content_length_limit(1024 * 16).and(warp::body::json())
}


pub fn with_db(pool: SqlitePool) -> impl Filter<Extract = (SqlitePool,), Error = Infallible> + Clone {
    warp::any().map(move || pool.clone())
}
