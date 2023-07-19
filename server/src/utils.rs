use std::time::{Duration, SystemTime};

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

fn current_system_time() -> anyhow::Result<Duration> {
    Ok(SystemTime::now().duration_since(std::time::UNIX_EPOCH)?)
}

pub fn timestamp_s() -> anyhow::Result<u64> {
    Ok(current_system_time()?.as_secs())
}

pub fn timestamp_us() -> anyhow::Result<u128> {
    Ok(current_system_time()?.as_micros())
}
