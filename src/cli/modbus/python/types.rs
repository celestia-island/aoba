use serde::{Deserialize, Serialize};

use crate::protocol::status::types::modbus::StationConfig;

/// Python execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PythonExecutionMode {
    /// Embedded RustPython VM
    Embedded,
    /// External CPython process
    External,
}

impl std::fmt::Display for PythonExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PythonExecutionMode::Embedded => write!(f, "embedded"),
            PythonExecutionMode::External => write!(f, "external"),
        }
    }
}

impl std::str::FromStr for PythonExecutionMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "embedded" | "rustpython" => Ok(PythonExecutionMode::Embedded),
            "external" | "cpython" => Ok(PythonExecutionMode::External),
            _ => Err(anyhow::anyhow!(
                "Invalid Python execution mode: '{}'. Use 'embedded' or 'external'",
                s
            )),
        }
    }
}

/// Output from Python script execution
#[derive(Debug, Clone)]
pub struct PythonOutput {
    /// Station configurations
    pub stations: Vec<StationConfig>,
    /// Optional reboot interval in milliseconds
    pub reboot_interval_ms: Option<u64>,
    /// Standard output messages (info level)
    pub stdout_messages: Vec<String>,
    /// Standard error messages (warning level)
    pub stderr_messages: Vec<String>,
}

impl PythonOutput {
    pub fn new(stations: Vec<StationConfig>) -> Self {
        Self {
            stations,
            reboot_interval_ms: None,
            stdout_messages: Vec::new(),
            stderr_messages: Vec::new(),
        }
    }

    pub fn with_reboot_interval(mut self, interval_ms: u64) -> Self {
        self.reboot_interval_ms = Some(interval_ms);
        self
    }

    pub fn add_stdout_message(&mut self, message: String) {
        self.stdout_messages.push(message);
    }

    pub fn add_stderr_message(&mut self, message: String) {
        self.stderr_messages.push(message);
    }
}

/// JSON format expected from Python scripts (compatible with both modes)
#[derive(Debug, Serialize, Deserialize)]
pub struct PythonScriptOutput {
    pub stations: Vec<StationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reboot_interval: Option<u64>,
}
