use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};

use super::{types::PythonOutput, PythonRunner};
use crate::protocol::status::types::modbus::StationConfig;

/// Embedded RustPython VM runner
/// NOTE: Currently disabled due to threading issues with RustPython 0.4
/// This will be re-enabled once RustPython supports better threading or we find a workaround
pub struct PythonEmbeddedRunner {
    script_path: String,
    reboot_interval_ms: u64,
    last_execution: Option<Instant>,
    active: bool,
}

impl PythonEmbeddedRunner {
    pub fn new(script_path: String, initial_reboot_interval_ms: Option<u64>) -> Result<Self> {
        log::warn!(
            "RustPython embedded mode is currently disabled. Please use external CPython mode instead."
        );
        log::warn!(
            "To use external mode, specify python:external:<path> instead of python:embedded:<path>"
        );

        Ok(Self {
            script_path,
            reboot_interval_ms: initial_reboot_interval_ms.unwrap_or(1000),
            last_execution: None,
            active: false,
        })
    }
}

impl PythonRunner for PythonEmbeddedRunner {
    fn execute(&mut self) -> Result<PythonOutput> {
        Err(anyhow!(
            "RustPython embedded mode is not currently supported. Please use external CPython mode (python:external:<path>)"
        ))
    }

    fn is_active(&self) -> bool {
        false
    }

    fn stop(&mut self) {
        self.active = false;
    }
}
