use std::time::{Duration, SystemTime};

use uuid::Uuid;

pub type Res<T> = Result<T, String>;

pub fn err<T, S: ToString>(message: S) -> Res<T> {
    Err(message.to_string())
}

#[derive(Clone, Copy)]
pub enum LogLevel {
    Error,
    Warning,
    Info,
    Debug,
}

impl LogLevel {
    fn to_str(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warning => " WARN",
            Self::Info => " INFO",
            Self::Debug => "DEBUG",
        }
    }
}

fn format_log_message<A: AsRef<str>>(level: LogLevel, message: A) -> String {
    format!(
        "[{}:{}] {}",
        level.to_str(),
        timestamp_s().unwrap_or(0),
        message.as_ref()
    )
}

pub fn log<A: AsRef<str>>(level: LogLevel, message: A) {
    let formatted = format_log_message(level, message);
    match level {
        LogLevel::Error => eprintln!("{}", formatted),
        _ => println!("{}", formatted),
    };
}

pub fn error<A: AsRef<str>>(message: A) {
    log(LogLevel::Error, message);
}

pub fn warning<A: AsRef<str>>(message: A) {
    log(LogLevel::Warning, message);
}

pub fn info<A: AsRef<str>>(message: A) {
    log(LogLevel::Info, message);
}

pub fn debug<A: AsRef<str>>(message: A) {
    log(LogLevel::Info, message);
}

fn current_system_time() -> Res<Duration> {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())
}

pub fn timestamp_s() -> Res<u64> {
    Ok(current_system_time()?.as_secs())
}

pub fn timestamp_us() -> Res<u128> {
    Ok(current_system_time()?.as_micros())
}

pub fn generate_uuid() -> Uuid {
    Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext))
}

pub fn format_uuid(uuid: Uuid) -> String {
    uuid.simple().to_string()
}

pub fn parse_uuid(text: &str) -> Res<Uuid> {
    Uuid::try_parse(text).map_err(|e| format!("Failed to parse UUID {text}: {e}"))
}
