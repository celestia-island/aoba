use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};

use super::{types::{PythonOutput, PythonScriptOutput}, PythonRunner};

/// External CPython process runner
pub struct PythonExternalRunner {
    script_path: String,
    python_command: String,
    reboot_interval_ms: u64,
    last_execution: Option<Instant>,
    active: bool,
}

impl PythonExternalRunner {
    pub fn new(script_path: String, initial_reboot_interval_ms: Option<u64>) -> Result<Self> {
        // Detect Python command location
        let python_command = Self::detect_python_command()?;
        log::info!("Detected Python command: {}", python_command);

        // Verify script exists
        if !std::path::Path::new(&script_path).exists() {
            return Err(anyhow!("Python script not found: {}", script_path));
        }

        Ok(Self {
            script_path,
            python_command,
            reboot_interval_ms: initial_reboot_interval_ms.unwrap_or(1000),
            last_execution: None,
            active: true,
        })
    }

    /// Detect Python command (python3 or python)
    fn detect_python_command() -> Result<String> {
        #[cfg(unix)]
        {
            // Try python3 first, then python
            if let Ok(output) = Command::new("which").arg("python3").output() {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() {
                        return Ok("python3".to_string());
                    }
                }
            }

            if let Ok(output) = Command::new("which").arg("python").output() {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() {
                        return Ok("python".to_string());
                    }
                }
            }
        }

        #[cfg(windows)]
        {
            // Try python first on Windows, then python3
            if let Ok(output) = Command::new("powershell")
                .args(["-Command", "Get-Command python -ErrorAction SilentlyContinue"])
                .output()
            {
                if output.status.success() && !output.stdout.is_empty() {
                    return Ok("python".to_string());
                }
            }

            if let Ok(output) = Command::new("powershell")
                .args(["-Command", "Get-Command python3 -ErrorAction SilentlyContinue"])
                .output()
            {
                if output.status.success() && !output.stdout.is_empty() {
                    return Ok("python3".to_string());
                }
            }
        }

        Err(anyhow!(
            "Python interpreter not found. Please install Python 3 and ensure it's in your PATH."
        ))
    }

    /// Parse JSON line from script output
    fn parse_json_line(line: &str) -> Result<PythonScriptOutput> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Err(anyhow!("Empty line"));
        }

        serde_json::from_str(trimmed).map_err(|e| anyhow!("Failed to parse JSON: {}", e))
    }
}

impl PythonRunner for PythonExternalRunner {
    fn execute(&mut self) -> Result<PythonOutput> {
        if !self.active {
            return Err(anyhow!("Runner is not active"));
        }

        // Check reboot interval
        if let Some(last_exec) = self.last_execution {
            let elapsed = last_exec.elapsed();
            let required = Duration::from_millis(self.reboot_interval_ms);
            if elapsed < required {
                return Err(anyhow!(
                    "Reboot interval not elapsed ({}ms remaining)",
                    (required - elapsed).as_millis()
                ));
            }
        }

        log::info!("Executing external Python script: {}", self.script_path);

        // Execute Python script
        let mut child = Command::new(&self.python_command)
            .arg(&self.script_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn Python process: {}", e))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to capture stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("Failed to capture stderr"))?;

        let mut output = PythonOutput::new(Vec::new());
        let mut found_valid_json = false;

        // Read stdout line by line
        let stdout_reader = BufReader::new(stdout);
        for line in stdout_reader.lines() {
            match line {
                Ok(line_str) => {
                    // Try to parse as JSON
                    match Self::parse_json_line(&line_str) {
                        Ok(script_output) => {
                            output.stations = script_output.stations;
                            if let Some(interval) = script_output.reboot_interval {
                                output.reboot_interval_ms = Some(interval);
                                self.reboot_interval_ms = interval;
                            }
                            found_valid_json = true;
                            log::info!(
                                "Parsed station data: {} stations",
                                output.stations.len()
                            );
                        }
                        Err(_) => {
                            // Not JSON, treat as info message
                            log::info!("Python script: {}", line_str);
                            output.add_stdout_message(line_str);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Error reading stdout: {}", e);
                }
            }
        }

        // Read stderr
        let stderr_reader = BufReader::new(stderr);
        for line in stderr_reader.lines() {
            match line {
                Ok(line_str) => {
                    log::warn!("Python script stderr: {}", line_str);
                    output.add_stderr_message(line_str);
                }
                Err(e) => {
                    log::warn!("Error reading stderr: {}", e);
                }
            }
        }

        // Wait for process to finish
        match child.wait() {
            Ok(status) => {
                if !status.success() {
                    log::warn!("Python script exited with status: {}", status);
                }
            }
            Err(e) => {
                log::warn!("Error waiting for Python process: {}", e);
            }
        }

        if !found_valid_json {
            return Err(anyhow!(
                "Python script did not produce valid JSON output on stdout"
            ));
        }

        self.last_execution = Some(Instant::now());
        Ok(output)
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn stop(&mut self) {
        self.active = false;
    }
}
