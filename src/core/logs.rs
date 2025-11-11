/// Logging infrastructure for core business logic
///
/// This module provides simple log entry types that can be used by
/// different UI frontends. UI-specific formatting and i18n should be
/// handled by the frontend.
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

/// A log entry with timestamp and message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Local>,
    pub message: String,
    pub level: LogLevel,
    pub metadata: Option<LogMetadata>,
}

/// Log entry severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

/// Additional metadata for structured logging
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LogMetadata {
    /// Subprocess lifecycle event
    Lifecycle {
        phase: LifecyclePhase,
        note: Option<String>,
    },
    /// Communication event (modbus, serial, etc)
    Communication {
        direction: CommunicationDirection,
        data: Vec<u8>,
        station_id: Option<u8>,
    },
    /// Management event (configuration change, etc)
    Management { event: ManagementEvent },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifecyclePhase {
    Created,
    Shutdown,
    Restarted,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommunicationDirection {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManagementEvent {
    ConfigurationApplied,
    ConfigurationDiscarded,
    PortEnabled,
    PortDisabled,
}

impl LogEntry {
    /// Create a new log entry with the current timestamp
    pub fn new(message: String, level: LogLevel) -> Self {
        Self {
            timestamp: Local::now(),
            message,
            level,
            metadata: None,
        }
    }

    /// Create a new log entry with metadata
    pub fn with_metadata(message: String, level: LogLevel, metadata: LogMetadata) -> Self {
        Self {
            timestamp: Local::now(),
            message,
            level,
            metadata: Some(metadata),
        }
    }

    /// Create an info-level log entry
    pub fn info(message: String) -> Self {
        Self::new(message, LogLevel::Info)
    }

    /// Create a warning-level log entry
    pub fn warning(message: String) -> Self {
        Self::new(message, LogLevel::Warning)
    }

    /// Create an error-level log entry
    pub fn error(message: String) -> Self {
        Self::new(message, LogLevel::Error)
    }
}

/// A simple log buffer that stores recent log entries
pub struct LogBuffer {
    entries: Vec<LogEntry>,
    max_entries: usize,
}

impl LogBuffer {
    /// Create a new log buffer with a maximum number of entries
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Add a log entry to the buffer
    pub fn push(&mut self, entry: LogEntry) {
        self.entries.push(entry);

        // Trim oldest entries if we exceed the max
        if self.entries.len() > self.max_entries {
            let excess = self.entries.len() - self.max_entries;
            self.entries.drain(0..excess);
        }
    }

    /// Get all log entries
    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }

    /// Get the number of log entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all log entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self::new(1000)
    }
}
