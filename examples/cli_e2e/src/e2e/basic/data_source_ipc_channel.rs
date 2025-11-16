use anyhow::{anyhow, Result};
use std::{io::Write, process::Stdio};

use crate::utils::{
    build_debug_bin, vcom_matchers_with_ports, wait_for_process_ready, DEFAULT_PORT1, DEFAULT_PORT2,
};
use _main::{cli::modbus::ModbusResponse, utils::sleep::sleep_1s};

// File-level constants to avoid magic numbers
const REGISTER_LENGTH: usize = 10;
const IPC_PIPE_PATH: &str = "/tmp/aoba_test_ipc_channel.fifo";

/// Test IPC channel data source - master provides data from IPC, slave polls and reads it
/// Tests 3 rounds of write-read cycles via IPC FIFO
pub async fn test_ipc_channel_data_source() -> Result<()> {
    log::info!("🧪 Testing IPC channel data source mode (master reads from IPC FIFO, slave polls master)...");
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

    let binary = build_debug_bin("aoba")?;
    let register_length_arg = REGISTER_LENGTH.to_string();

    // Create IPC FIFO (named pipe)
    log::info!("📁 Creating IPC FIFO at {}", IPC_PIPE_PATH);
    let _ = std::fs::remove_file(IPC_PIPE_PATH);
    
    #[cfg(unix)]
    {
        // Use mkfifo command to create the FIFO
        let output = std::process::Command::new("mkfifo")
            .arg(IPC_PIPE_PATH)
            .output()?;
        if !output.status.success() {
            return Err(anyhow!(
                "Failed to create FIFO: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    // Start master with IPC data source on port2
    let master_output = temp_dir.join("master_ipc_persist_output.log");
    let master_output_file = std::fs::File::create(&master_output)?;
    let master_stderr = temp_dir.join("master_ipc_persist_stderr.log");
    let master_stderr_file = std::fs::File::create(&master_stderr)?;

    log::info!(
        "📋 Master logs will be at: stdout={:?}, stderr={:?}",
        master_output,
        master_stderr
    );

    let ipc_data_source = format!("ipc:{}", IPC_PIPE_PATH);
    let mut master = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--master-provide-persist",
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
            "--data-source",
            &ipc_data_source,
        ])
        .stdout(Stdio::from(master_output_file))
        .stderr(Stdio::from(master_stderr_file))
        .spawn()?;

    // Wait for master to be ready and create the FIFO
    wait_for_process_ready(&mut master, 3000).await?;
    sleep_1s().await;

    // Start slave poll persist on port1 to continuously read from master
    let slave_output = temp_dir.join("slave_ipc_poll_output.log");
    let slave_output_file = std::fs::File::create(&slave_output)?;
    let slave_stderr = temp_dir.join("slave_ipc_poll_stderr.log");
    let slave_stderr_file = std::fs::File::create(&slave_stderr)?;

    log::info!(
        "📋 Slave logs will be at: stdout={:?}, stderr={:?}",
        slave_output,
        slave_stderr
    );

    let mut slave = std::process::Command::new(&binary)
        .arg("--enable-virtual-ports")
        .args([
            "--slave-poll-persist",
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
            "--request-interval-ms",
            "2000",
        ])
        .stdout(Stdio::from(slave_output_file))
        .stderr(Stdio::from(slave_stderr_file))
        .spawn()?;

    // Wait for slave to be ready
    wait_for_process_ready(&mut slave, 3000).await?;
    sleep_1s().await;

    // Test Round 1: Sequential values
    log::info!("🔄 Round 1: Writing values to IPC FIFO");
    let json_values1 = serde_json::json!({"values": round1_values}).to_string();
    log::info!("📝 Writing to FIFO: {}", json_values1);
    
    // Write to FIFO in a separate thread to avoid blocking
    let fifo_path1 = IPC_PIPE_PATH.to_string();
    let json_values1_clone = json_values1.clone();
    std::thread::spawn(move || {
        if let Ok(mut fifo) = std::fs::OpenOptions::new().write(true).open(&fifo_path1) {
            let _ = writeln!(fifo, "{}", json_values1_clone);
            let _ = fifo.flush();
        }
    });
    
    log::info!("✅ Data written to FIFO, waiting for slave to poll...");
    sleep_1s().await;
    sleep_1s().await;
    sleep_1s().await;
    
    // Read slave output to get the polled values
    log::info!("🔍 Round 1: Reading slave output");
    let slave_content1 = std::fs::read_to_string(&slave_output)?;
    let lines1: Vec<&str> = slave_content1.lines().collect();
    let received1 = if let Some(last_line) = lines1.last() {
        log::info!("📥 Last slave output line: {}", last_line);
        let response: ModbusResponse = serde_json::from_str(last_line)?;
        response.values
    } else {
        master.kill().ok();
        slave.kill().ok();
        let _ = master.wait();
        let _ = slave.wait();
        return Err(anyhow!("Round 1: No output from slave"));
    };
    
    log::info!("✅ Round 1: Received values: {:?}", received1);
    if received1 != round1_values {
        master.kill().ok();
        slave.kill().ok();
        let _ = master.wait();
        let _ = slave.wait();
        return Err(anyhow!(
            "Round 1: Received values {:?} do not match expected {:?}",
            received1,
            round1_values
        ));
    }

    // Test Round 2: Reverse values
    log::info!("🔄 Round 2: Writing values to IPC FIFO");
    let json_values2 = serde_json::json!({"values": round2_values}).to_string();
    log::info!("📝 Writing to FIFO: {}", json_values2);
    
    let fifo_path2 = IPC_PIPE_PATH.to_string();
    let json_values2_clone = json_values2.clone();
    std::thread::spawn(move || {
        if let Ok(mut fifo) = std::fs::OpenOptions::new().write(true).open(&fifo_path2) {
            let _ = writeln!(fifo, "{}", json_values2_clone);
            let _ = fifo.flush();
        }
    });
    
    log::info!("✅ Data written to FIFO, waiting for slave to poll...");
    sleep_1s().await;
    sleep_1s().await;
    sleep_1s().await;
    
    log::info!("🔍 Round 2: Reading slave output");
    let slave_content2 = std::fs::read_to_string(&slave_output)?;
    let lines2: Vec<&str> = slave_content2.lines().collect();
    let received2 = if let Some(last_line) = lines2.last() {
        log::info!("📥 Last slave output line: {}", last_line);
        let response: ModbusResponse = serde_json::from_str(last_line)?;
        response.values
    } else {
        master.kill().ok();
        slave.kill().ok();
        let _ = master.wait();
        let _ = slave.wait();
        return Err(anyhow!("Round 2: No output from slave"));
    };
    
    log::info!("✅ Round 2: Received values: {:?}", received2);
    if received2 != round2_values {
        master.kill().ok();
        slave.kill().ok();
        let _ = master.wait();
        let _ = slave.wait();
        return Err(anyhow!(
            "Round 2: Received values {:?} do not match expected {:?}",
            received2,
            round2_values
        ));
    }

    // Test Round 3: Custom hex values
    log::info!("🔄 Round 3: Writing values to IPC FIFO");
    let json_values3 = serde_json::json!({"values": round3_values}).to_string();
    log::info!("📝 Writing to FIFO: {}", json_values3);
    
    let fifo_path3 = IPC_PIPE_PATH.to_string();
    let json_values3_clone = json_values3.clone();
    std::thread::spawn(move || {
        if let Ok(mut fifo) = std::fs::OpenOptions::new().write(true).open(&fifo_path3) {
            let _ = writeln!(fifo, "{}", json_values3_clone);
            let _ = fifo.flush();
        }
    });
    
    log::info!("✅ Data written to FIFO, waiting for slave to poll...");
    sleep_1s().await;
    sleep_1s().await;
    sleep_1s().await;
    
    log::info!("🔍 Round 3: Reading slave output");
    let slave_content3 = std::fs::read_to_string(&slave_output)?;
    let lines3: Vec<&str> = slave_content3.lines().collect();
    let received3 = if let Some(last_line) = lines3.last() {
        log::info!("📥 Last slave output line: {}", last_line);
        let response: ModbusResponse = serde_json::from_str(last_line)?;
        response.values
    } else {
        master.kill().ok();
        slave.kill().ok();
        let _ = master.wait();
        let _ = slave.wait();
        return Err(anyhow!("Round 3: No output from slave"));
    };
    
    log::info!("✅ Round 3: Received values: {:?}", received3);
    if received3 != round3_values {
        master.kill().ok();
        slave.kill().ok();
        let _ = master.wait();
        let _ = slave.wait();
        return Err(anyhow!(
            "Round 3: Received values {:?} do not match expected {:?}",
            received3,
            round3_values
        ));
    }

    // Cleanup
    master.kill().ok();
    slave.kill().ok();
    let _ = master.wait();
    let _ = slave.wait();
    let _ = std::fs::remove_file(IPC_PIPE_PATH);
    std::fs::remove_file(&master_output).ok();
    std::fs::remove_file(&slave_output).ok();

    log::info!("✅ IPC channel data source test completed successfully");
    Ok(())
}
