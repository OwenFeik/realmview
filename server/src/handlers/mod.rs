use std::convert::Infallible;

use sqlx::SqlitePool;
use warp::Filter;

mod login;
mod logout;
mod media;
mod media_details;
mod register;
mod upload;

pub fn routes(
    pool: SqlitePool,
    content_dir: String,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    login::filter(pool.clone())
        .or(register::filter(pool.clone()))
        .or(logout::filter(pool.clone()))
        .or(upload::filter(pool.clone(), content_dir))
        .or(media::filter(pool.clone()))
        .or(media_details::filter(pool))
}

pub fn json_body<T: std::marker::Send + for<'de> serde::Deserialize<'de>>(
) -> impl Filter<Extract = (T,), Error = warp::Rejection> + Clone {
    warp::body::content_length_limit(1024 * 16).and(warp::body::json())
}

pub fn with_db(
    pool: SqlitePool,
) -> impl Filter<Extract = (SqlitePool,), Error = Infallible> + Clone {
    warp::any().map(move || pool.clone())
}

pub fn current_time() -> anyhow::Result<u64> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs())
}

fn parse_cookie(cookies: String, goal_key: &str) -> Option<String> {
    for cookie in cookies.split(';') {
        let parts = cookie.splitn(2, '=').collect::<Vec<&str>>();
        if let Some(key) = parts.get(0) {
            if key.trim() == goal_key {
                return parts.get(1).map(|s| String::from(s.trim()));
            }
        }
    }

    None
}

pub fn with_session() -> impl Filter<Extract = (Option<String>,), Error = warp::Rejection> + Clone {
    warp::filters::header::optional::<String>("Cookie").map(|c: Option<String>| match c {
        Some(s) => parse_cookie(s, "session_key"),
        None => None,
    })
}

pub async fn session_user(
    pool: &SqlitePool,
    session_key: Option<String>,
) -> Result<models::User, response::ResultReply> {
    match session_key {
        Some(k) => match models::UserSession::get(pool, &k).await {
            Ok(Some(session)) => match session.user(pool).await {
                Ok(Some(user)) => Ok(user),
                Ok(None) => Err(response::Binary::result_failure("User not found.")),
                Err(_) => Err(response::Binary::result_error("Database error.")),
            },
            Ok(None) => Err(response::Binary::result_failure("Session not found.")),
            Err(_) => Err(response::Binary::result_failure("Database error.")),
        },
        None => Err(response::Binary::result_failure("Session required.")),
    }
}

pub mod response {
    use std::convert::Infallible;

    use serde::Serialize;
    use serde_derive::Serialize;
    use warp::http::StatusCode;

    type JsonReply = warp::reply::WithStatus<warp::reply::Json>;
    pub type ResultReply = Result<JsonReply, Infallible>;

    #[derive(Serialize)]
    pub struct Binary {
        message: String,
        success: bool,
    }

    impl Binary {
        pub fn new(message: &str, success: bool) -> Binary {
            Binary {
                message: String::from(message),
                success,
            }
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
            as_result(
                &Binary::new_failure(message),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    }

    pub fn as_reply(body: &impl Serialize, status: StatusCode) -> JsonReply {
        warp::reply::with_status(warp::reply::json(body), status)
    }

    pub fn as_result(body: &impl Serialize, status: StatusCode) -> ResultReply {
        Ok(as_reply(body, status))
    }

    pub fn cookie_result(
        body: &impl Serialize,
        status: StatusCode,
        key: &str,
        value: Option<&str>,
    ) -> Result<impl warp::Reply, Infallible> {
        let cookie = value.unwrap_or("");

        // SameSite=Strict causes the cookie to be sent only on requests from
        // this website to this website.
        //
        // Max-Age=15552000 causes the cookie to be retained for up to 6 months
        // unless cleared (manually or by logging out).
        let cookie = format!("{}={}; SameSite=Strict; Max-Age=15552000;", key, cookie);

        Ok(warp::reply::with_header(
            as_reply(&body, status),
            "Set-Cookie",
            cookie.as_str(),
        ))
    }
}

pub mod crypto {
    use std::fmt::Write;
    use std::num::NonZeroU32;

    use ring::{
        pbkdf2,
        rand::{SecureRandom, SystemRandom},
    };

    const KEY_LENGTH: usize = ring::digest::SHA256_OUTPUT_LEN;
    pub type Key = [u8; KEY_LENGTH];

    pub fn generate_salt() -> anyhow::Result<Key> {
        let mut bytes = [0u8; KEY_LENGTH];
        let rng = SystemRandom::new();
        match rng.fill(&mut bytes) {
            Ok(()) => Ok(bytes),
            Err(_) => Err(anyhow::anyhow!("Random byte generation failed.")),
        }
    }

    pub fn to_hex_string(key: &Key) -> anyhow::Result<String> {
        let mut s = String::with_capacity(KEY_LENGTH * 2);
        for byte in *key {
            write!(s, "{:02X}", byte)?;
        }

        Ok(s)
    }

    pub fn to_hex_string_unsized(data: &[u8]) -> anyhow::Result<String> {
        let key =
            &<Key>::try_from(data).map_err(|_| anyhow::anyhow!("Failed to convert Vec to Key."))?;
        to_hex_string(key)
    }

    pub fn from_hex_string(string: &str) -> anyhow::Result<Key> {
        let err = Err(anyhow::anyhow!("Failed to decode hex."));
        match ring::test::from_hex(string) {
            Ok(v) => Ok(v.try_into().or(err)?),
            Err(_) => err,
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
            &mut hashed,
        );

        hashed
    }

    pub fn check_password(provided: &str, salt: &Key, hashed_password: &Key) -> bool {
        pbkdf2::verify(
            pbkdf2::PBKDF2_HMAC_SHA256,
            NonZeroU32::new(ITERATIONS).unwrap(),
            salt,
            provided.as_bytes(),
            hashed_password,
        )
        .is_ok()
    }
}

pub mod models {
    use sqlx::{FromRow, SqlitePool};

    #[derive(FromRow)]
    pub struct User {
        pub id: i64,
        pub username: String,
        pub salt: String,
        pub hashed_password: String,
        pub recovery_key: String,
        pub created_time: i64,
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

        pub fn relative_dir(&self) -> String {
            format!("uploads/{}", &self.username)
        }

        pub fn upload_dir(&self, content_dir: &str) -> String {
            format!("{}/{}", content_dir, &self.relative_dir())
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
        pub end_time: Option<i64>,
    }

    impl UserSession {
        pub async fn get(
            pool: &SqlitePool,
            session_key: &str,
        ) -> anyhow::Result<Option<UserSession>> {
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
