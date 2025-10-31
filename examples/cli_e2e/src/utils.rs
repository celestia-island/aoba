use anyhow::Result;
use std::process::Command;

/// Factory function to create a Modbus command
///
/// # Arguments
/// * `is_slave` - true for slave (server), false for master (client)
/// * `port` - Serial port path (e.g., "/tmp/vcom1")
/// * `is_persist` - true for persistent mode, false for temporary mode
/// * `output_or_source` - Optional output file for slave or data source file for master
///
/// # Returns
/// A Command that can be further configured and executed
#[allow(dead_code)]
pub fn create_modbus_command(
    is_slave: bool,
    port: &str,
    is_persist: bool,
    output_or_source: Option<&str>,
) -> Result<Command> {
    let binary = aoba_ci_utils::build_debug_bin("aoba")?;
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

    let mut cmd = Command::new(binary);
    cmd.args(args.iter());
    Ok(cmd)
}
