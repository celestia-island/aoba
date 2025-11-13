pub mod embedded;
pub mod external;
pub mod types;

pub use embedded::PythonEmbeddedRunner;
pub use external::PythonExternalRunner;
pub use types::{PythonExecutionMode, PythonOutput};

use anyhow::Result;

/// Trait for Python script execution
pub trait PythonRunner: Send {
    /// Execute the Python script and return the output
    fn execute(&mut self) -> Result<PythonOutput>;

    /// Check if the runner is still active
    fn is_active(&self) -> bool;

    /// Stop the runner
    fn stop(&mut self);
}

/// Create a Python runner based on the execution mode
pub fn create_python_runner(
    mode: PythonExecutionMode,
    script_path: String,
    initial_reboot_interval_ms: Option<u64>,
) -> Result<Box<dyn PythonRunner>> {
    match mode {
        PythonExecutionMode::Embedded => Ok(Box::new(PythonEmbeddedRunner::new(
            script_path,
            initial_reboot_interval_ms,
        )?)),
        PythonExecutionMode::External => Ok(Box::new(PythonExternalRunner::new(
            script_path,
            initial_reboot_interval_ms,
        )?)),
    }
}
