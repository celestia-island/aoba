pub mod embedded;
pub mod types;

use std::path::{Path, PathBuf};

use anyhow::Result;

pub use embedded::PythonEmbeddedRunner;
pub use types::PythonOutput;

/// Trait for Python script execution
pub trait PythonRunner: Send {
    /// Execute the Python script and return the output
    fn execute(&mut self) -> Result<PythonOutput>;

    /// Check if the runner is still active
    fn is_active(&self) -> bool;

    /// Stop the runner
    fn stop(&mut self);
}

/// Resolve a script path: if relative, make it relative to the current working directory
fn resolve_script_path(script_path: &str) -> PathBuf {
    let path = Path::new(script_path);
    
    // If the path is already absolute, return it as-is
    if path.is_absolute() {
        return path.to_path_buf();
    }
    
    // For relative paths, resolve relative to the current working directory
    match std::env::current_dir() {
        Ok(cwd) => {
            let resolved = cwd.join(path);
            log::debug!(
                "Resolved relative Python script path: '{}' -> '{}'",
                script_path,
                resolved.display()
            );
            resolved
        }
        Err(e) => {
            log::warn!(
                "Failed to get current directory for Python script path resolution: {}. Using path as-is: '{}'",
                e,
                script_path
            );
            path.to_path_buf()
        }
    }
}

/// Create a Python runner based on the execution mode
pub fn create_python_runner(
    script_path: String,
    initial_reboot_interval_ms: Option<u64>,
) -> Result<Box<dyn PythonRunner>> {
    // Resolve relative paths to absolute paths based on current working directory
    let resolved_path = resolve_script_path(&script_path);
    let resolved_path_str = resolved_path.to_string_lossy().to_string();
    
    Ok(Box::new(PythonEmbeddedRunner::new(
        resolved_path_str,
        initial_reboot_interval_ms,
    )?))
}
