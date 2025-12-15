// Authorship: Human 100% | Claude 0%
use chrono::{DateTime, Utc};

#[allow(dead_code)]
#[derive(Debug)]
pub enum NesLogLevel {
    Info,
    Warning,
    Error,
}
#[allow(dead_code)]
#[derive(Debug)]
pub struct NesLog<T> {
    timestamp: DateTime<Utc>,
    severity: NesLogLevel,
    payload: T,
}

impl<T> NesLog<T> {
    pub fn new(severity: NesLogLevel, payload: T) -> NesLog<T> {
        NesLog {
            timestamp: Utc::now(),
            severity,
            payload,
        }
    }
}

impl From<String> for NesLog<String> {
    fn from(message: String) -> Self {
        NesLog::new(NesLogLevel::Info, message)
    }
}