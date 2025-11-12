use anyhow::{anyhow, Result};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::{net::TcpListener, process::Stdio, sync::Arc, thread, time::Duration};
use tokio::task;

use tiny_http::{Header, Response, Server};
use ureq::Agent;

use crate::utils::{
    build_debug_bin, sleep_1s, vcom_matchers_with_ports, DEFAULT_PORT1, DEFAULT_PORT2,
};
use aoba::{
    cli::modbus::ModbusResponse,
    protocol::status::types::modbus::{RegisterMode, StationConfig, StationMode},
};

async fn run_simple_server(payload: Arc<Vec<StationConfig>>) -> Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    listener.set_nonblocking(false)?;
    let server = Server::from_listener(listener, None)
        .map_err(|err| anyhow!("Failed to start HTTP server: {err}"))?;

    let serialized_payload = serde_json::to_string(payload.as_ref())?;
    log::info!("HTTP server will serve: {}", serialized_payload);
    let response_payload = Arc::new(serialized_payload);

    thread::spawn(move || {
        for request in server.incoming_requests() {
            let mut response = Response::from_string(response_payload.as_ref().clone());
            if let Ok(header) = Header::from_bytes(b"Content-Type", b"application/json") {
                response.add_header(header);
            }
            if let Err(err) = request.respond(response) {
                log::error!("HTTP server respond error: {err}");
            }
        }
    });

    let url = format!("http://{}", addr);
    let url_clone = url.clone();
    task::spawn_blocking(move || {
        let agent = Agent::new_with_defaults();
        for _ in 0..20 {
            match agent.get(&url_clone).call() {
                Ok(resp) if resp.status() == 200 => return Ok(()),
                _ => thread::sleep(Duration::from_millis(50)),
            }
        }
        Err(anyhow!("HTTP server failed to respond in time"))
    })
    .await??;

    Ok(url)
}

fn build_station_payload(values: &[u16]) -> Arc<Vec<StationConfig>> {
    Arc::new(vec![StationConfig::single_range(
        1,
        StationMode::Master,
        RegisterMode::Holding,
        0,
        10,
        Some(values.to_vec()),
    )])
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

/// Test master mode with HTTP data source using tiny_http for the test server
pub async fn test_http_data_source() -> Result<()> {
    log::info!("🧪 Testing HTTP data source mode...");
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    let mut rng = StdRng::seed_from_u64(0xA0BA_DA7A_01_u64);
    let expected_values: Vec<u16> = (0..10).map(|_| rng.random::<u16>()).collect();
    let payload = build_station_payload(&expected_values);
    let server_url = run_simple_server(payload).await?;

    log::info!("🧪 Starting HTTP test server on {}", server_url);

    let server_output = temp_dir.join("server_http_output.log");
    let server_output_file = std::fs::File::create(&server_output)?;

    let binary = build_debug_bin("aoba")?;
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
            "10",
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--data-source",
            &server_url,
        ])
        .stdout(Stdio::from(server_output_file))
        .stderr(Stdio::piped())
        .spawn()?;

    sleep_1s().await;
    sleep_1s().await;
    sleep_1s().await;

    // Check if master is still running before starting client
    if let Some(status) = master.try_wait()? {
        let stderr_content = if let Some(mut stderr) = master.stderr.take() {
            use std::io::Read;
            let mut buf = String::new();
            stderr.read_to_string(&mut buf).ok();
            buf
        } else {
            String::new()
        };
        
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow!(
            "Master exited prematurely with status {status}. Stderr: {stderr_content}"
        ));
    }
    
    log::info!("✅ Master is still running, starting slave poll...");

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
            "10",
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?
        .wait_with_output()?;

    // Read master stdout even if it exited
    let master_stdout_path = server_output.clone();
    let master_stdout_content = std::fs::read_to_string(&master_stdout_path).unwrap_or_else(|_| String::from("Could not read master stdout"));
    log::info!("Master stdout: {}", master_stdout_content);

    let master_status = master.wait()?;
    log::info!("Master exit status: {}", master_status);
    if !master_status.success() {
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow!("Master exited with status {master_status}"));
    }

    std::fs::remove_file(&server_output).ok();

    if !client_output.status.success() {
        // Check if master is still running
        let master_still_running = master.try_wait()?.is_none();
        
        // Capture master stderr for debugging
        let master_stderr = if let Some(mut stderr) = master.stderr.take() {
            use std::io::Read;
            let mut buf = String::new();
            stderr.read_to_string(&mut buf).ok();
            buf
        } else {
            String::new()
        };
        
        std::fs::remove_file(&server_output).ok();
        
        return Err(anyhow!(
            "Slave poll command failed: {} (stderr: {})\nMaster still running: {}\nMaster stderr: {}",
            client_output.status,
            String::from_utf8_lossy(&client_output.stderr),
            master_still_running,
            master_stderr
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

    log::info!("✅ HTTP data source test passed");
    Ok(())
}

/// Test master mode with HTTP data source in persistent mode using tiny_http
pub async fn test_http_data_source_persist() -> Result<()> {
    log::info!("🧪 Testing HTTP data source persistent mode...");
    let ports = vcom_matchers_with_ports(DEFAULT_PORT1, DEFAULT_PORT2);
    let temp_dir = std::env::temp_dir();

    let mut rng = StdRng::seed_from_u64(0xA0BA_DA7A_02_u64);
    let expected_values: Vec<u16> = (0..10).map(|_| rng.random::<u16>()).collect();
    let payload = build_station_payload(&expected_values);
    let server_url = run_simple_server(payload).await?;

    log::info!("🧪 Starting HTTP test server on {}", server_url);

    let server_output = temp_dir.join("server_http_persist_output.log");
    let server_output_file = std::fs::File::create(&server_output)?;

    let binary = build_debug_bin("aoba")?;
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
            "10",
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
            "--data-source",
            &server_url,
        ])
        .stdout(Stdio::from(server_output_file))
        .stderr(Stdio::piped())
        .spawn()?;

    sleep_1s().await;
    sleep_1s().await;
    sleep_1s().await;

    if let Some(status) = master.try_wait()? {
        std::fs::remove_file(&server_output).ok();
        return Err(anyhow!("Master exited prematurely with status {status}"));
    }

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
            "10",
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
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

    log::info!("✅ HTTP data source persistent test passed");
    Ok(())
}
