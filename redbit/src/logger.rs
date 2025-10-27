use chrono::Utc; // <- was Local
use std::{
    env, fmt,
    sync::{atomic::{AtomicU8, Ordering}, Once},
};

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Level { Off = 0, Error = 1, Warn = 2, Info = 3 }

static LOG_LEVEL: AtomicU8 = AtomicU8::new(Level::Info as u8);
static INIT: Once = Once::new();

fn parse_level(s: &str) -> Option<Level> {
    match s.trim().to_ascii_lowercase().as_str() {
        "off" => Some(Level::Off),
        "error" => Some(Level::Error),
        "warn" | "warning" => Some(Level::Warn),
        "info" => Some(Level::Info),
        other => {
            // tolerate "crate=level,foo=info" by picking first valid level token
            for part in other.split(',') {
                if let Some(l) = parse_level(part.rsplit('=').next().unwrap_or(part)) {
                    return Some(l);
                }
            }
            None
        }
    }
}

fn init_from_env_once() {
    INIT.call_once(|| {
        if let Ok(val) = env::var("RUST_LOG") {
            if let Some(lvl) = parse_level(&val) {
                LOG_LEVEL.store(lvl as u8, Ordering::Relaxed);
            }
        }
    });
}

fn enabled(threshold: Level) -> bool {
    init_from_env_once();
    LOG_LEVEL.load(Ordering::Relaxed) >= threshold as u8
}

pub fn info(args: fmt::Arguments) {
    if enabled(Level::Info) {
        let now = Utc::now(); // cheaper than Local
        println!("[{}] INFO {}", now.format("%Y-%m-%d %H:%M:%S"), args);
    }
}

pub fn warn(args: fmt::Arguments) {
    if enabled(Level::Warn) {
        let now = Utc::now();
        eprintln!("[{}] WARN {}", now.format("%Y-%m-%d %H:%M:%S"), args);
    }
}

pub fn error(args: fmt::Arguments) {
    if enabled(Level::Error) {
        let now = Utc::now();
        eprintln!("[{}] ERROR {}", now.format("%Y-%m-%d %H:%M:%S"), args);
    }
}

// macros unchanged
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => { $crate::logger::info(format_args!($($arg)*)) };
}
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => { $crate::logger::warn(format_args!($($arg)*)) };
}
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => { $crate::logger::error(format_args!($($arg)*)) };
}
