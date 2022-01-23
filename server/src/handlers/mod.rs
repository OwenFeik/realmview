use std::convert::Infallible;

use sqlx::SqlitePool;
use warp::Filter;


mod login;
mod logout;
mod register;
mod upload;


pub fn filters(pool: SqlitePool, content_dir: String)
    -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone
{
    login::filter(pool.clone())
        .or(register::filter(pool.clone()))
        .or(logout::filter(pool.clone()))
        .or(upload::filter(pool, content_dir))
}


pub fn json_body<T: std::marker::Send + for<'de> serde::Deserialize<'de>>() ->
    impl Filter<Extract = (T,), Error = warp::Rejection> + Clone
{
    warp::body::content_length_limit(1024 * 16).and(warp::body::json())
}


pub fn with_db(pool: SqlitePool) -> impl Filter<Extract = (SqlitePool,), Error = Infallible> + Clone {
    warp::any().map(move || pool.clone())
}


pub fn current_time() -> anyhow::Result<u64> {
    Ok(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs())
}


fn parse_cookie(cookies: String, goal_key: &str) -> Option<String> {
    for cookie in cookies.split(";") {
        let parts = cookie.splitn(2, "=").collect::<Vec<&str>>();
        if let Some(key) = parts.get(0) {
            if key.trim() == goal_key {
                return parts.get(1).map(|s| String::from(s.trim()));
            }
        }
    }

    None
}


pub fn with_session() -> impl Filter<Extract = (Option<String>,), Error = warp::Rejection> + Clone {
    warp::filters::header::optional::<String>("Cookie")
        .map(|c: Option<String>| {
            match c {
                Some(s) => parse_cookie(s, "session_key"),
                None => None
            }
        })
}


pub mod response {
    use std::convert::Infallible;

    use serde::Serialize;
    use serde_derive::Serialize;
    use warp::http::StatusCode;


    type JsonReply = warp::reply::WithStatus<warp::reply::Json>;
    type ResultReply = Result<JsonReply, Infallible>;

    #[derive(Serialize)]
    pub struct Binary {
        message: String,
        success: bool
    }

    impl Binary {
        pub fn new(message: &str, success: bool) -> Binary {
            Binary { message: String::from(message), success }
        }
    
        pub fn new_success(message: &str) -> Binary {
            Binary::new(message, true)
        }
    
        pub fn new_failure(message: &str) -> Binary {
            Binary::new(message, false)
        }

        pub fn result_success(message: &str) -> ResultReply {
            as_result(&Binary::new_success(message), StatusCode::OK)
        }
    
        pub fn result_failure(message: &str) -> ResultReply {
            as_result(&Binary::new_failure(message), StatusCode::OK)
        }

        pub fn result_error(message: &str) -> ResultReply {
            as_result(&Binary::new_failure(message), StatusCode::INTERNAL_SERVER_ERROR)
        }
    }

    pub fn as_reply(body: &impl Serialize, status: StatusCode) -> JsonReply {
        warp::reply::with_status(warp::reply::json(body), status)
    }

    pub fn as_result(body: &impl Serialize, status: StatusCode) -> ResultReply {
        Ok(as_reply(body, status))
    }

    pub fn cookie_result(body: &impl Serialize, status: StatusCode, key: &str, value: Option<&str>)
        -> Result<impl warp::Reply, Infallible>
    {
        let cookie = match value {
            Some(s) => s,
            None => ""
        };
        let cookie = format!("{}={}", key, cookie);

        Ok(warp::reply::with_header(as_reply(&body, status), "Set-Cookie", cookie.as_str()))
    }
}


pub mod crypto {
    use std::fmt::Write;
    use std::num::NonZeroU32;

    use ring::{pbkdf2, rand::{SecureRandom, SystemRandom}};

    const KEY_LENGTH: usize = ring::digest::SHA512_OUTPUT_LEN;
    pub type Key = [u8; KEY_LENGTH];

    pub fn generate_salt() -> anyhow::Result<Key> {
        let mut bytes = [0u8; KEY_LENGTH];
        let rng = SystemRandom::new();
        match rng.fill(&mut bytes) {
            Ok(()) => Ok(bytes),
            Err(_) => Err(anyhow::anyhow!("Random byte generation failed."))
        }
    }

    pub fn to_hex_string(key: &Key) -> anyhow::Result<String> {
        let mut s = String::with_capacity(KEY_LENGTH * 2);
        for byte in *key {
            write!(s, "{:02X}", byte)?;
        }
    
        Ok(s)
    }

    pub fn from_hex_string(string: &str) -> anyhow::Result<Key> {
        let err = Err(anyhow::anyhow!("Failed to decode hex."));
        match ring::test::from_hex(string) {
            Ok(v) => Ok(v.try_into().or(err)?),
            Err(_) => err
        }
    }

    pub fn random_hex_string(length: usize) -> anyhow::Result<String> {
        Ok(to_hex_string(&generate_salt()?)?[..length].to_string())
    }

    const ITERATIONS: u32 = 10_000;
    pub fn hash_password(salt: &Key, password: &str) -> Key {
        let mut hashed = [0u8; KEY_LENGTH];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            NonZeroU32::new(ITERATIONS).unwrap(),
            salt,
            password.as_bytes(),
            &mut hashed
        );

        hashed
    }

    pub fn check_password(provided: &str, salt: &Key, hashed_password: &Key) -> bool {
        pbkdf2::verify(
            pbkdf2::PBKDF2_HMAC_SHA256,
            NonZeroU32::new(ITERATIONS).unwrap(),
            salt,
            provided.as_bytes(),
            hashed_password
        ).is_ok()
    }
}


pub mod models {
    use sqlx::{SqlitePool, FromRow};

    #[derive(FromRow)]
    pub struct User {
        pub id: i64,
        pub username: String,
        pub salt: String,
        pub hashed_password: String,
        pub recovery_key: String,
        pub created_time: i64
    }

    impl User {
        pub async fn get(pool: &SqlitePool, username: &str) -> anyhow::Result<Option<User>> {
            let user = sqlx::query_as("SELECT * FROM users WHERE username = ?1;")
                .bind(username)    
                .fetch_optional(pool)
                .await?;
            Ok(user)
        }

        pub async fn get_by_id(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<User>> {
            let user = sqlx::query_as("SELECT * FROM users WHERE id = ?1;")
                .bind(id)
                .fetch_optional(pool)
                .await?;
            Ok(user)
        }

        pub fn upload_dir(&self, content_dir: &str) -> String {
            format!("{}/uploads/{}", content_dir, self.username.as_str())
        }
    }

    #[derive(FromRow)]
    pub struct UserSession {
        pub id: i64,

        #[sqlx(rename = "user")]
        pub user_id: i64,

        pub session_key: String,
        pub active: bool,
        pub start_time: i64,
        pub end_time: Option<i64>
    }

    impl UserSession {
        pub async fn get(pool: &SqlitePool, session_key: &str) -> anyhow::Result<Option<UserSession>> {
            let user_sesion = sqlx::query_as("SELECT * FROM user_sessions WHERE session_key = ?1;")
            .bind(session_key)
            .fetch_optional(pool)
            .await?;
            Ok(user_sesion)
        }

        pub async fn user(&self, pool: &SqlitePool) -> anyhow::Result<Option<User>> {
            User::get_by_id(pool, self.user_id).await
        }
    }
}
