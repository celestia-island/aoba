use crate::protocol::status::types::modbus::StationConfig;

/// Output from Python script execution
#[derive(Debug, Clone)]
pub struct PythonOutput {
    /// Station configurations
    pub stations: Vec<StationConfig>,
    /// Optional reboot interval in milliseconds
    pub reboot_interval_ms: Option<u64>,
}

impl PythonOutput {
    pub fn new(stations: Vec<StationConfig>) -> Self {
        Self {
            stations,
            reboot_interval_ms: None,
        }
    }

    pub fn with_reboot_interval(mut self, interval_ms: u64) -> Self {
        self.reboot_interval_ms = Some(interval_ms);
        self
    }
}
