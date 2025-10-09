use anyhow::{anyhow, Result};
use std::process::Stdio;

/// Factory function to create a Modbus command.
pub fn create_modbus_command(
    is_slave: bool,
    port: &str,
    is_persist: bool,
    output_or_source: Option<&str>,
) -> Result<std::process::Command> {
    let binary = crate::terminal::build_debug_bin("aoba")?;
    let mode = if is_persist { "-persist" } else { "" };
    let mut args: Vec<String> = vec![
        format!(
            "--{}{}",
            if is_slave {
                "slave-listen"
            } else {
                "master-provide"
            },
            mode
        ),
        port.to_string(),
        "--station-id".to_string(),
        "1".to_string(),
        "--register-address".to_string(),
        "0".to_string(),
        "--register-length".to_string(),
        "5".to_string(),
        "--register-mode".to_string(),
        "holding".to_string(),
        "--baud-rate".to_string(),
        "9600".to_string(),
    ];

    if let Some(out_src) = output_or_source {
        if is_slave {
            args.push("--output".to_string());
        } else {
            args.push("--data-source".to_string());
        }
        args.push(out_src.to_string());
    }

    let mut cmd = std::process::Command::new(binary);
    cmd.args(args.iter());
    Ok(cmd)
}

/// Run a simple CLI poll command (non-persistent) for convenience in tests.
pub async fn run_cli_slave_poll() -> Result<String> {
    let binary = crate::terminal::build_debug_bin("aoba")?;

    let output = std::process::Command::new(&binary)
        .args([
            "--slave-poll",
            "/tmp/vcom2",
            "--baud-rate",
            "9600",
            "--station-id",
            "1",
            "--register-mode",
            "holding",
            "--register-address",
            "0",
            "--register-length",
            "12",
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(anyhow!(format!("CLI command failed: {stderr}")));
    }

    Ok(stdout)
}
