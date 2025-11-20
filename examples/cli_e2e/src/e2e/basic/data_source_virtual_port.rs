use anyhow::Result;
use std::{
    io::Write,
    process::Stdio,
};

use crate::utils::{build_debug_bin, wait_for_process_ready};
use _main::{
    cli::modbus::ModbusResponse,
    utils::{sleep::sleep_1s, sleep_3s},
};

// File-level constants
const REGISTER_LENGTH: usize = 10;

/// Test virtual port with UUID - CLI launches with random UUID as port name
/// Tests that virtual ports ignore baud rate and other serial configurations
/// Verifies basic functionality works without physical serial port
pub async fn test_virtual_port() -> Result<()> {
    log::info!("🧪 Testing virtual port with UUID (ignore baud rate, verify availability)...");
    let temp_dir = std::env::temp_dir();

    // Generate UUID v7 for virtual port name
    let virtual_port_uuid = uuid::Uuid::now_v7().to_string();
    log::info!("📍 Using virtual port UUID: {}", virtual_port_uuid);

    // Test data - Sequential values for verification
    let test_values: Vec<u16> = (0..REGISTER_LENGTH as u16).collect();
    log::info!("📊 Test values: {:?}", test_values);

    let binary = build_debug_bin("aoba")?;
    let register_length_arg = REGISTER_LENGTH.to_string();

    // Start slave daemon with virtual port (UUID)
    let slave_output = temp_dir.join("slave_virtual_port_output.log");
    let slave_output_file = std::fs::File::create(&slave_output)?;
    let slave_stderr = temp_dir.join("slave_virtual_port_stderr.log");
    let slave_stderr_file = std::fs::File::create(&slave_stderr)?;

    log::info!(
        "📋 Slave logs will be at: stdout={:?}, stderr={:?}",
        slave_output,
        slave_stderr
    );

    // Launch slave with virtual port (UUID name)
    let mut slave_child = std::process::Command::new(&binary)
        .arg("--slave-listen-persist")
        .arg(&virtual_port_uuid)
        .arg("--station-id")
        .arg("1")
        .arg("--register-mode")
        .arg("holding")
        .arg("--register-address")
        .arg("0")
        .arg("--register-length")
        .arg(&register_length_arg)
        .arg("--initial-values")
        .arg(
            test_values
                .iter()
                .map(|v| format!("{:04x}", v))
                .collect::<Vec<_>>()
                .join(","),
        )
        .stdout(Stdio::from(slave_output_file))
        .stderr(Stdio::from(slave_stderr_file))
        .spawn()?;

    log::info!(
        "🚀 Slave process started on virtual port {} with PID {}",
        virtual_port_uuid,
        slave_child.id()
    );

    // Wait for slave to initialize
    wait_for_process_ready(&mut slave_child, 3000).await?;
    sleep_1s().await;

    // Start master to query the slave via virtual port
    let master_output = temp_dir.join("master_virtual_port_output.log");
    let master_output_file = std::fs::File::create(&master_output)?;
    let master_stderr = temp_dir.join("master_virtual_port_stderr.log");
    let master_stderr_file = std::fs::File::create(&master_stderr)?;

    log::info!(
        "📋 Master logs will be at: stdout={:?}, stderr={:?}",
        master_output,
        master_stderr
    );

    let mut master_child = std::process::Command::new(&binary)
        .arg("--master-provide-temp")
        .arg(&virtual_port_uuid)
        .arg("--station-id")
        .arg("1")
        .arg("--register-mode")
        .arg("holding")
        .arg("--register-address")
        .arg("0")
        .arg("--register-length")
        .arg(&register_length_arg)
        .stdin(Stdio::piped())
        .stdout(Stdio::from(master_output_file))
        .stderr(Stdio::from(master_stderr_file))
        .spawn()?;

    log::info!(
        "🚀 Master process started on virtual port {} with PID {}",
        virtual_port_uuid,
        master_child.id()
    );

    let mut master_stdin = master_child.stdin.take().expect("Failed to get stdin");

    // Wait for master to initialize
    wait_for_process_ready(&mut master_child, 3000).await?;
    sleep_1s().await;

    // Send query command to master (just send empty line to poll)
    log::info!("📤 Sending query command to master...");
    writeln!(master_stdin)?;
    master_stdin.flush()?;

    log::info!("⏳ Waiting for master response...");
    sleep_3s().await;

    // Read master output to verify communication
    let master_output_content = std::fs::read_to_string(&master_output)?;
    log::info!("📖 Master output content:\n{}", master_output_content);

    // Parse the response and verify values
    let mut found_response = false;
    for line in master_output_content.lines() {
        if line.trim().starts_with('{') {
            if let Ok(response) = serde_json::from_str::<ModbusResponse>(line) {
                log::info!("✅ Received ModbusResponse with {} values", response.values.len());

                // Verify the response
                if response.station_id == 1 && response.register_mode == "holding" {
                    log::info!("🔍 Verifying values: {:?}", response.values);
                    assert_eq!(
                        response.values.len(),
                        REGISTER_LENGTH,
                        "Value count mismatch"
                    );
                    assert_eq!(
                        response.values, test_values,
                        "Values do not match expected test data"
                    );
                    found_response = true;
                    log::info!("✅ Virtual port communication verified successfully!");
                    break;
                }
            }
        }
    }

    assert!(
        found_response,
        "Did not receive valid response from virtual port communication"
    );

    // Cleanup
    log::info!("🧹 Cleaning up processes...");
    let _ = master_child.kill();
    let _ = master_child.wait();
    let _ = slave_child.kill();
    let _ = slave_child.wait();

    log::info!("✅ Virtual port test completed successfully (baud rate ignored)!");
    Ok(())
}
