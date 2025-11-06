use anyhow::Result;
use std::{
    process::{Command, Stdio},
    time::Duration,
};

use crate::utils::build_debug_bin;
use aoba_cli::config::{
    CommunicationMethod, CommunicationParams, ModbusBootConfig, PersistenceMode, RegisterMap,
    RegisterRange, StationConfig, StationMode,
};

/// Test multiple slaves configuration
pub async fn test_multi_slaves() -> Result<()> {
    log::info!("🧪 Testing multiple slaves configuration...");

    // Create configuration using the type-safe ModbusBootConfig struct
    let config = ModbusBootConfig {
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
                station_id: 1,
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
                station_id: 2,
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
                station_id: 3,
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
    let binary = build_debug_bin("aoba")?;

    // Start configuration mode
    log::info!("🧪 Starting multi-slaves configuration...");
    let mut process = Command::new(&binary)
        .arg("--config")
        .arg(&config_file)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Wait a bit to allow the process to start and initialize
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Check if process is still running (config mode runs persistently)
    match process.try_wait()? {
        None => {
            // Process is still running, which is expected for persistent config mode
            log::info!("✅ Multi-slaves configuration process started successfully and is running");
        }
        Some(status) => {
            // Process exited unexpectedly
            let output = process.wait_with_output()?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            log::error!("❌ Process exited prematurely with status: {status}");
            log::error!("stdout: {stdout}");
            log::error!("stderr: {stderr}");

            // Clean up and return error
            std::fs::remove_file(&config_file)?;
            return Err(anyhow::anyhow!(
                "Multi-slaves configuration process exited prematurely"
            ));
        }
    }

    // Kill the process since it runs persistently
    process.kill()?;
    process.wait()?;
    log::info!("✅ Stopped multi-slaves configuration process");

    // Clean up temporary files
    std::fs::remove_file(&config_file)?;

    log::info!("✅ Multi-slaves test completed successfully");
    Ok(())
}
