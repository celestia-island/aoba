use anyhow::{anyhow, Result};
use std::{process::Stdio, time::Duration};

use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};

use crate::utils::{
    build_debug_bin, vcom_matchers_with_ports, wait_for_process_ready, DEFAULT_PORT1, DEFAULT_PORT2,
};
use _main::{
    api::modbus::ModbusResponse,
    protocol::status::types::modbus::{RegisterMode, StationConfig, StationMode},
    utils::{sleep_1s, sleep_3s},
};

// File-level constant for register length used in tests
const REGISTER_LENGTH: usize = 10;

fn build_station_payload(values: &[u16]) -> Vec<StationConfig> {
    vec![StationConfig::single_range(
        1,
        StationMode::Master,
        RegisterMode::Holding,
        0,
        REGISTER_LENGTH as u16,
        Some(values.to_vec()),
    )]
}

fn parse_client_response(stdout: &[u8]) -> Result<ModbusResponse> {
    let output = String::from_utf8_lossy(stdout);
    let response_line = output
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| anyhow!("Client produced empty stdout"))?;
    let response: ModbusResponse = serde_json::from_str(response_line)
        .map_err(|err| anyhow!("Failed to parse ModbusResponse JSON: {err}"))?;
    Ok(response)
}

/// Publish data to MQTT broker for testing
async fn publish_mqtt_data(
    broker_host: &str,
    broker_port: u16,
    topic: &str,
    payload: &str,
) -> Result<()> {
    let client_id = format!("test_publisher_{}", uuid::Uuid::new_v4());
    let mut mqtt_options = MqttOptions::new(client_id, broker_host, broker_port);
    mqtt_options.set_keep_alive(Duration::from_secs(5));

    let (client, mut event_loop) = AsyncClient::new(mqtt_options, 10);

    // Spawn event loop in background
    let event_handle = tokio::spawn(async move {
        loop {
            match event_loop.poll().await {
                Ok(Event::Incoming(Incoming::ConnAck(_))) => {
        
                }
                Err(e) => {
                    log::warn!("MQTT event loop error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });

    // Wait for connection
    sleep_1s().await;

    // Publish the payload with retained flag so the subscriber can receive it even if it connects later
    client
        .publish(topic, QoS::AtLeastOnce, true, payload.as_bytes())
        .await
        .map_err(|e| anyhow!("Failed to publish MQTT message: {}", e))?;

    log::info!("Published MQTT data to topic: {}", topic);

    // Give time for message to be delivered
    sleep_1s().await;

    // Clean up
    client.disconnect().await.ok();
    event_handle.abort();

    Ok(())
}

/// Test master mode with MQTT data source
/// This test requires an external MQTT broker to be running (e.g., mosquitto)
/// Tests 3 rounds of data updates to verify continuous MQTT message reception
pub async fn test_mqtt_data_source() -> Result<()> {
    log::info!("🧪 Testing MQTT data source mode (3 rounds of data updates)...");

    // Check if mosquitto broker is available
    let broker_available = std::process::Command::new("sh")
        .arg("-c")
        .arg("pgrep mosquitto || command -v mosquitto")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !broker_available {
        log::warn!("⚠️ Mosquitto MQTT broker not found. Attempting to install and start it...");

        // Try to install mosquitto if not available
        let install_result = std::process::Command::new("sh")
            .arg("-c")
            .arg("command -v mosquitto || (sudo apt-get update && sudo apt-get install -y mosquitto)")
            .output();

        if let Ok(output) = install_result {
            if !output.status.success() {
                log::warn!(
                    "Failed to install mosquitto: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        // Kill any existing mosquitto instances on port 1883
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg("pkill -9 -f 'mosquitto.*1883' || true")
            .output();

        sleep_1s().await;

        // Try to start mosquitto in the background with explicit config
        let start_result = std::process::Command::new("mosquitto")
            .args(["-p", "1883", "-v"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        match start_result {
            Ok(child) => {
                // Detach the child process so it keeps running
                let _ = child.id();
                log::info!("Started mosquitto broker on port 1883");
            }
            Err(e) => {
                log::error!("Failed to start mosquitto: {}", e);
                return Err(anyhow!(
                    "Mosquitto broker is required but could not be started: {}",
                    e
                ));
            }
        }

        // Wait longer for broker to be ready
        sleep_3s().await;
    }

    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    // Round 1: Sequential values
    let round1_values: Vec<u16> = (0..REGISTER_LENGTH as u16).collect();
    log::info!("📊 Round 1 expected values: {:?}", round1_values);

    // Round 2: Reverse values
    let round2_values: Vec<u16> = (0..REGISTER_LENGTH as u16).rev().collect();
    log::info!("📊 Round 2 expected values: {:?}", round2_values);

    // Round 3: Custom hex values
    let round3_values: Vec<u16> = vec![
        0x1111, 0x2222, 0x3333, 0x4444, 0x5555, 0x6666, 0x7777, 0x8888, 0x9999, 0xAAAA,
    ];
    log::info!("📊 Round 3 expected values: {:?}", round3_values);

    let payload_round1 = build_station_payload(&round1_values);

    let topic = "aoba/test/data_persist";
    let broker_host = "127.0.0.1";
    let broker_port = 1883;
    let mqtt_url = format!("mqtt://{}:{}/{}", broker_host, broker_port, topic);

    log::info!("🧪 Using MQTT broker at {}", mqtt_url);

    let server_output = temp_dir.join("server_mqtt_persist_output.log");
    let server_output_file = std::fs::File::create(&server_output)?;
    let server_stderr = temp_dir.join("server_mqtt_persist_stderr.log");
    let server_stderr_file = std::fs::File::create(&server_stderr)?;

    let binary = build_debug_bin("aoba")?;
    let register_length_arg = REGISTER_LENGTH.to_string();

    log::info!(
        "📋 Master logs will be at: stdout={:?}, stderr={:?}",
        server_output,
        server_stderr
    );

    let mut master = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--master-provide-persist",
            &ports.port1_name,
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            &register_length_arg,
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--data-source",
            &mqtt_url,
        ])
        .stdout(Stdio::from(server_output_file))
        .stderr(Stdio::from(server_stderr_file))
        .spawn()?;

    // Wait for master to be ready and MQTT connection to establish
    wait_for_process_ready(&mut master, 5000).await?;

    // Test Round 1: Sequential values
    log::info!("🔄 Round 1: Publishing sequential values to MQTT topic");
    let serialized_payload = serde_json::to_string(&payload_round1)?;
    publish_mqtt_data(broker_host, broker_port, topic, &serialized_payload).await?;
    sleep_1s().await;

    log::info!("🔍 Round 1: Polling data from slave");
    let client_output = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--slave-poll",
            &ports.port2_name,
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            &register_length_arg,
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--timeout-ms",
            "10000",
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?
        .wait_with_output()?;

    if !client_output.status.success() {
        let stderr = String::from_utf8_lossy(&client_output.stderr);
        master.kill().ok();
        let _ = master.wait();
        std::fs::remove_file(&server_output).ok();
        std::fs::remove_file(&server_stderr).ok();
        return Err(anyhow!(
            "Round 1: Slave poll command failed: {} (stderr: {})",
            client_output.status,
            stderr
        ));
    }

    let response = parse_client_response(&client_output.stdout)?;
    log::info!("✅ Round 1: Received values: {:?}", response.values);
    if response.values != round1_values {
        master.kill().ok();
        let _ = master.wait();
        std::fs::remove_file(&server_output).ok();
        std::fs::remove_file(&server_stderr).ok();
        return Err(anyhow!(
            "Round 1: Received values {:?} do not match expected {:?}",
            response.values,
            round1_values
        ));
    }

    // Test Round 2: Reverse values
    log::info!("🔄 Round 2: Publishing reverse values to MQTT topic");
    let payload_round2 = build_station_payload(&round2_values);
    let serialized_payload = serde_json::to_string(&payload_round2)?;
    publish_mqtt_data(broker_host, broker_port, topic, &serialized_payload).await?;
    sleep_1s().await;

    log::info!("🔍 Round 2: Polling data from slave");
    let client_output = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--slave-poll",
            &ports.port2_name,
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            &register_length_arg,
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--timeout-ms",
            "10000",
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?
        .wait_with_output()?;

    if !client_output.status.success() {
        let stderr = String::from_utf8_lossy(&client_output.stderr);
        master.kill().ok();
        let _ = master.wait();
        std::fs::remove_file(&server_output).ok();
        std::fs::remove_file(&server_stderr).ok();
        return Err(anyhow!(
            "Round 2: Slave poll command failed: {} (stderr: {})",
            client_output.status,
            stderr
        ));
    }

    let response = parse_client_response(&client_output.stdout)?;
    log::info!("✅ Round 2: Received values: {:?}", response.values);
    if response.values != round2_values {
        master.kill().ok();
        let _ = master.wait();
        std::fs::remove_file(&server_output).ok();
        std::fs::remove_file(&server_stderr).ok();
        return Err(anyhow!(
            "Round 2: Received values {:?} do not match expected {:?}",
            response.values,
            round2_values
        ));
    }

    // Test Round 3: Custom hex values
    log::info!("🔄 Round 3: Publishing custom hex values to MQTT topic");
    let payload_round3 = build_station_payload(&round3_values);
    let serialized_payload = serde_json::to_string(&payload_round3)?;
    publish_mqtt_data(broker_host, broker_port, topic, &serialized_payload).await?;
    sleep_1s().await;

    log::info!("🔍 Round 3: Polling data from slave");
    let client_output = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--slave-poll",
            &ports.port2_name,
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            &register_length_arg,
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--timeout-ms",
            "10000",
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?
        .wait_with_output()?;

    if !client_output.status.success() {
        let stderr = String::from_utf8_lossy(&client_output.stderr);
        master.kill().ok();
        let _ = master.wait();
        std::fs::remove_file(&server_output).ok();
        std::fs::remove_file(&server_stderr).ok();
        return Err(anyhow!(
            "Round 3: Slave poll command failed: {} (stderr: {})",
            client_output.status,
            stderr
        ));
    }

    let response = parse_client_response(&client_output.stdout)?;
    log::info!("✅ Round 3: Received values: {:?}", response.values);
    if response.values != round3_values {
        master.kill().ok();
        let _ = master.wait();
        std::fs::remove_file(&server_output).ok();
        std::fs::remove_file(&server_stderr).ok();
        return Err(anyhow!(
            "Round 3: Received values {:?} do not match expected {:?}",
            response.values,
            round3_values
        ));
    }

    if let Some(status) = master.try_wait()? {
        master.kill().ok();
        let _ = master.wait();
        std::fs::remove_file(&server_output).ok();
        std::fs::remove_file(&server_stderr).ok();
        return Err(anyhow!(
            "Master exited after handling poll with status {status}"
        ));
    }

    master.kill().ok();
    let _ = master.wait();
    std::fs::remove_file(&server_output).ok();
    std::fs::remove_file(&server_stderr).ok();

    log::info!("✅ MQTT data source test passed (all 3 rounds verified)");
    Ok(())
}
