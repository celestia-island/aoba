use chrono::{DateTime, Local};

use crate::protocol::status::ParsedRequest;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub when: DateTime<Local>,
    pub raw: String,
    pub parsed: Option<ParsedRequest>,
}

pub const LOG_GROUP_HEIGHT: usize = 3; // fallback constant
