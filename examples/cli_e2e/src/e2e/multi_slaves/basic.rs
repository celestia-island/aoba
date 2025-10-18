use anyhow::Result;
use std::{
    process::{Command, Stdio},
    time::Duration,
};

use _bin::cli::config::{
    CommunicationMethod, CommunicationParams, Config, PersistenceMode, RegisterRange,
    RegisterType, StationConfig, StationMode, RegisterMap,
};

/// Test multiple slaves configuration
pub async fn test_multi_slaves() -> Result<()> {
    log::info!("🧪 Testing multiple slaves configuration...");

    // Create configuration using the type-safe Config struct
    let config = Config {
        port_name: "/tmp/vcom2".to_string(),
        baud_rate: 9600,
        communication_params: CommunicationParams {
            mode: CommunicationMethod::Stdio,
            dynamic_pull: false,
            wait_time: Some(1.0),
            timeout: Some(3.0),
            persistence: PersistenceMode::Persistent,
        },
        stations: vec![
            StationConfig {
                id: 1,
                mode: StationMode::Slave,
                map: RegisterMap {
                    holding: vec![RegisterRange {
                        address_start: 0,
                        length: 10,
                        initial_values: vec![],
                    }],
                    ..Default::default()
                },
            },
            StationConfig {
                id: 2,
                mode: StationMode::Slave,
                map: RegisterMap {
                    input: vec![RegisterRange {
                        address_start: 100,
                        length: 5,
                        initial_values: vec![],
                    }],
                    ..Default::default()
                },
            },
            StationConfig {
                id: 3,
                mode: StationMode::Slave,
                map: RegisterMap {
                    coils: vec![RegisterRange {
                        address_start: 50,
                        length: 8,
                        initial_values: vec![],
                    }],
                    ..Default::default()
                },
            },
        ],
    };

    // Convert configuration to a JSON string
    let config_json = serde_json::to_string_pretty(&config)?;

    // Write the configuration to a temporary file
    let temp_dir = std::env::temp_dir();
    let config_file = temp_dir.join("test_multi_slaves.json");
    std::fs::write(&config_file, config_json)?;

    log::info!("🧪 Created configuration file: {config_file:?}");

    // Build the binary
    let binary = ci_utils::build_debug_bin("aoba")?;

    // Start configuration mode
    log::info!("🧪 Starting multi-slaves configuration...");
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
        log::info!("✅ Multi-slaves configuration completed successfully");

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
            "⚠️ Multi-slaves configuration failed with status: {}",
            output.status
        );
        log::warn!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        log::warn!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        return Err(anyhow::anyhow!("Multi-slaves configuration failed"));
    }

    // Clean up temporary files
    std::fs::remove_file(&config_file)?;

    log::info!("✅ Multi-slaves test completed successfully");
    Ok(())
}
