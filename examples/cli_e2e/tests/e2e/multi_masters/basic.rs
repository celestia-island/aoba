use anyhow::Result;
use std::{
    process::{Command, Stdio},
    time::Duration,
};

use aoba::cli::config::{
    CommunicationMethod, CommunicationMode, CommunicationParams, Config, ModbusRegister,
    PersistenceMode, RegisterType,
};

/// Test multiple masters configuration
pub async fn test_multi_masters() -> Result<()> {
    log::info!("🧪 Testing multiple masters configuration...");

    // Create configuration using the type-safe Config struct
    let config = Config {
        port_name: "/tmp/vcom1".to_string(),
        baud_rate: 9600,
        communication_mode: CommunicationMode::Master,
        communication_params: CommunicationParams {
            mode: CommunicationMethod::Stdio,
            dynamic_pull: false,
            wait_time: Some(1.0),
            timeout: Some(3.0),
            persistence: PersistenceMode::Persistent,
        },
        modbus_configs: vec![
            ModbusRegister {
                station_id: 1,
                register_type: RegisterType::Holding,
                start_address: 0,
                length: 10,
            },
            ModbusRegister {
                station_id: 2,
                register_type: RegisterType::Input,
                start_address: 100,
                length: 5,
            },
            ModbusRegister {
                station_id: 3,
                register_type: RegisterType::Coils,
                start_address: 50,
                length: 8,
            },
        ],
    };

    // Convert configuration to a JSON string
    let config_json = serde_json::to_string_pretty(&config)?;

    // Write the configuration to a temporary file
    let temp_dir = std::env::temp_dir();
    let config_file = temp_dir.join("test_multi_masters.json");
    std::fs::write(&config_file, config_json)?;

    log::info!("🧪 Created configuration file: {config_file:?}");

    // Build the binary
    let binary = ci_utils::build_debug_bin("aoba")?;

    // Start configuration mode
    log::info!("🧪 Starting multi-masters configuration...");
    let process = Command::new(&binary)
        .arg("--config")
        .arg(&config_file)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Wait a bit to allow the process to start
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Wait for the process to complete
    let output = process.wait_with_output()?;

    // Check whether the process exited successfully
    if output.status.success() {
        log::info!("✅ Multi-masters configuration completed successfully");

        // Check whether the output contains the configuration loaded successfully message
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if stdout.contains("Configuration loaded successfully")
            || stderr.contains("Configuration loaded successfully")
        {
            log::info!("✅ Configuration loading message found");
        } else {
            log::warn!("⚠️ Configuration loading message not found in output");
            log::debug!("stdout: {stdout}");
            log::debug!("stderr: {stderr}");
        }
    } else {
        log::warn!(
            "⚠️ Multi-masters configuration failed with status: {}",
            output.status
        );
        log::warn!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        log::warn!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        return Err(anyhow::anyhow!("Multi-masters configuration failed"));
    }

    // Clean up temporary files
    std::fs::remove_file(&config_file)?;

    log::info!("✅ Multi-masters test completed successfully");
    Ok(())
}
