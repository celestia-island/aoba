use anyhow::{anyhow, Result};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::{process::Stdio, time::Duration};

use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};

use crate::utils::{
    build_debug_bin, vcom_matchers_with_ports, wait_for_process_ready, DEFAULT_PORT1, DEFAULT_PORT2,
};
use aoba::{
    cli::modbus::ModbusResponse,
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
                    log::debug!("MQTT publisher connected");
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

/// Test master mode with MQTT data source (non-persistent mode)
/// This test requires an external MQTT broker to be running (e.g., mosquitto)
pub async fn test_mqtt_data_source() -> Result<()> {
    log::info!("🧪 Testing MQTT data source mode...");

    // Check if mosquitto broker is available
    let broker_available = std::process::Command::new("sh")
        .arg("-c")
        .arg("pgrep mosquitto || command -v mosquitto")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !broker_available {
        log::warn!("⚠️ Mosquitto MQTT broker not found. Installing or starting it...");
        // Try to start mosquitto in the background
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg("mosquitto -d -p 1883 2>/dev/null || true")
            .spawn();
        sleep_3s().await;
    }

    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    let mut rng = StdRng::seed_from_u64(0x00A0_BADA_7A03_u64);
    let expected_values: Vec<u16> = (0..REGISTER_LENGTH).map(|_| rng.random::<u16>()).collect();
    let payload = build_station_payload(&expected_values);

    let topic = "aoba/test/data";
    let broker_host = "127.0.0.1";
    let broker_port = 1883;
    let mqtt_url = format!("mqtt://{}:{}/{}", broker_host, broker_port, topic);

    log::info!("🧪 Using MQTT broker at {}", mqtt_url);

    // Publish test data to MQTT broker
    let serialized_payload = serde_json::to_string(&payload)?;
    publish_mqtt_data(broker_host, broker_port, topic, &serialized_payload).await?;

    let server_output = temp_dir.join("server_mqtt_output.log");
    let server_output_file = std::fs::File::create(&server_output)?;

    let binary = build_debug_bin("aoba")?;
    let register_length_arg = REGISTER_LENGTH.to_string();

    let mut master = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--master-provide",
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
        .stderr(Stdio::piped())
        .spawn()?;

    // Wait for master to be ready and receive MQTT data (minimum 5 seconds for MQTT connection)
    wait_for_process_ready(&mut master, 5000).await?;

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
            "10000", // 10 second timeout to account for serial communication delays
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?
        .wait_with_output()?;

    let master_status = master.wait()?;
    if !master_status.success() {
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow!("Master exited with status {master_status}"));
    }

    std::fs::remove_file(&server_output).ok();

    if !client_output.status.success() {
        return Err(anyhow!(
            "Slave poll command failed: {} (stderr: {})",
            client_output.status,
            String::from_utf8_lossy(&client_output.stderr)
        ));
    }

    let response = parse_client_response(&client_output.stdout)?;
    if response.values != expected_values {
        return Err(anyhow!(
            "Received values {:?} do not match expected {:?}",
            response.values,
            expected_values
        ));
    }

    log::info!("✅ MQTT data source test passed");
    Ok(())
}

/// Test master mode with MQTT data source in persistent mode
pub async fn test_mqtt_data_source_persist() -> Result<()> {
    log::info!("🧪 Testing MQTT data source persistent mode...");

    // Check if mosquitto broker is available
    let broker_available = std::process::Command::new("sh")
        .arg("-c")
        .arg("pgrep mosquitto || command -v mosquitto")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !broker_available {
        log::warn!("⚠️ Mosquitto MQTT broker not found. Installing or starting it...");
        // Try to start mosquitto in the background
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg("mosquitto -d -p 1883 2>/dev/null || true")
            .spawn();
        sleep_3s().await;
    }

    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    let mut rng = StdRng::seed_from_u64(0x00A0_BADA_7A04_u64);
    let expected_values: Vec<u16> = (0..REGISTER_LENGTH).map(|_| rng.random::<u16>()).collect();
    let payload = build_station_payload(&expected_values);

    let topic = "aoba/test/data_persist";
    let broker_host = "127.0.0.1";
    let broker_port = 1883;
    let mqtt_url = format!("mqtt://{}:{}/{}", broker_host, broker_port, topic);

    log::info!("🧪 Using MQTT broker at {}", mqtt_url);

    // Publish test data to MQTT broker
    let serialized_payload = serde_json::to_string(&payload)?;
    publish_mqtt_data(broker_host, broker_port, topic, &serialized_payload).await?;

    let server_output = temp_dir.join("server_mqtt_persist_output.log");
    let server_output_file = std::fs::File::create(&server_output)?;

    let binary = build_debug_bin("aoba")?;
    let register_length_arg = REGISTER_LENGTH.to_string();

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
        .stderr(Stdio::piped())
        .spawn()?;

    // Wait for master to be ready and receive MQTT data (minimum 5 seconds for MQTT connection)
    wait_for_process_ready(&mut master, 5000).await?;

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
            "10000", // 10 second timeout to account for serial communication delays
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
        return Err(anyhow!(
            "Slave poll command failed: {} (stderr: {})",
            client_output.status,
            stderr
        ));
    }

    let response = parse_client_response(&client_output.stdout)?;
    if response.values != expected_values {
        master.kill().ok();
        let _ = master.wait();
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow!(
            "Received values {:?} do not match expected {:?}",
            response.values,
            expected_values
        ));
    }

    if let Some(status) = master.try_wait()? {
        master.kill().ok();
        let _ = master.wait();
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow!(
            "Master exited after handling poll with status {status}"
        ));
    }

    master.kill().ok();
    let _ = master.wait();
    std::fs::remove_file(&server_output).ok();

    log::info!("✅ MQTT data source persistent test passed");
    Ok(())
}
