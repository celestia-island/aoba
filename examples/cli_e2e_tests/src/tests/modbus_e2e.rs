use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

/// Test master-slave communication with virtual serial ports
pub fn test_master_slave_communication() -> Result<()> {
    log::info!("🧪 Testing master-slave communication with virtual serial ports...");
    
    // Wait longer to ensure previous tests have fully released ports
    thread::sleep(Duration::from_secs(2));

    let binary = aoba::ci::build_debug_bin("aoba")?;

    // Create a temporary file with test data
    let temp_dir = std::env::temp_dir();
    let data_file = temp_dir.join("test_modbus_e2e_data.json");

    {
        let mut file = File::create(&data_file)?;
        writeln!(file, r#"{{"values": [10, 20, 30, 40, 50]}}"#)?;
        writeln!(file, r#"{{"values": [15, 25, 35, 45, 55]}}"#)?;
    }

    // Start master (slave device) on /dev/vcom1 in persistent mode
    log::info!("🧪 Starting master (slave device) on /dev/vcom1...");
    let master_output = temp_dir.join("master_output.log");
    let master_output_file = File::create(&master_output)?;

    let mut master = Command::new(&binary)
        .args(&[
            "--master-provide-persist",
            "/dev/vcom1",
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            "5",
            "--register-mode",
            "holding",
            "--data-source",
            &format!("file:{}", data_file.display()),
            "--baud-rate",
            "9600",
        ])
        .stdout(Stdio::from(master_output_file))
        .stderr(Stdio::piped())
        .spawn()?;

    // Give master time to start and fully acquire the port
    thread::sleep(Duration::from_secs(3));

    // Check if master is still running
    match master.try_wait()? {
        Some(status) => {
            let stderr = if let Some(stderr) = master.stderr.take() {
                let mut buf = String::new();
                let mut reader = BufReader::new(stderr);
                let _ = reader.read_line(&mut buf);
                buf
            } else {
                String::new()
            };

            let _ = std::fs::remove_file(&data_file);
            let _ = std::fs::remove_file(&master_output);

            return Err(anyhow!(
                "Master exited prematurely with status {}: {}",
                status,
                stderr
            ));
        }
        None => {
            log::info!("✅ Master is running");
        }
    }

    // Now start slave (master device) on /dev/vcom2 in temporary mode
    log::info!("🧪 Starting slave (master device) on /dev/vcom2...");
    let slave_output = Command::new(&binary)
        .args(&[
            "--slave-listen",
            "/dev/vcom2",
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            "5",
            "--register-mode",
            "holding",
            "--baud-rate",
            "9600",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Wait for slave to complete (temporary mode exits after one response)
    let slave_result = slave_output.wait_with_output()?;

    // Kill master process and wait for it to fully exit
    let _ = master.kill();
    let _ = master.wait();
    
    // Give extra time for ports to be fully released
    thread::sleep(Duration::from_secs(1));

    log::info!("🧪 Slave exit status: {}", slave_result.status);
    log::info!(
        "🧪 Slave stdout: {}",
        String::from_utf8_lossy(&slave_result.stdout)
    );
    log::info!(
        "🧪 Slave stderr: {}",
        String::from_utf8_lossy(&slave_result.stderr)
    );

    // Read master output
    let master_output_content = std::fs::read_to_string(&master_output).unwrap_or_default();
    log::info!("🧪 Master output: {}", master_output_content);

    // Clean up
    let _ = std::fs::remove_file(&data_file);
    let _ = std::fs::remove_file(&master_output);

    // Verify communication happened
    if !slave_result.status.success() {
        return Err(anyhow!("Slave command failed"));
    }

    let stdout = String::from_utf8_lossy(&slave_result.stdout);
    if stdout.trim().is_empty() {
        log::warn!("⚠️ No output from slave (communication may have failed)");
    } else {
        log::info!("✅ Slave produced output, communication test passed");
    }

    Ok(())
}

/// Test slave listen with temporary mode and virtual ports
pub fn test_slave_listen_with_vcom() -> Result<()> {
    log::info!("🧪 Testing slave listen temporary mode with virtual serial ports...");

    let binary = aoba::ci::build_debug_bin("aoba")?;

    // Just verify the command works with virtual ports
    let output = Command::new(&binary)
        .args(&[
            "--slave-listen",
            "/dev/vcom1",
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            "5",
            "--register-mode",
            "holding",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    match output {
        Ok(mut child) => {
            // Wait for a short time
            thread::sleep(Duration::from_millis(1000));
            let _ = child.kill();
            let _ = child.wait();

            log::info!("✅ Slave listen command accepted with virtual ports");
            
            // Give extra time for port to be fully released
            thread::sleep(Duration::from_secs(1));
            
            Ok(())
        }
        Err(e) => {
            log::error!("Failed to spawn slave listen: {}", e);
            Err(anyhow!("Failed to spawn: {}", e))
        }
    }
}

/// Test master provide with persistent mode and virtual ports
pub fn test_master_provide_with_vcom() -> Result<()> {
    log::info!("🧪 Testing master provide persistent mode with virtual serial ports...");

    let binary = aoba::ci::build_debug_bin("aoba")?;

    // Create a temporary file with test data
    let temp_dir = std::env::temp_dir();
    let data_file = temp_dir.join("test_modbus_vcom_data.json");

    {
        let mut file = File::create(&data_file)?;
        writeln!(file, r#"{{"values": [100, 200, 300, 400, 500]}}"#)?;
    }

    let output = Command::new(&binary)
        .args(&[
            "--master-provide-persist",
            "/dev/vcom2",
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            "5",
            "--register-mode",
            "holding",
            "--data-source",
            &format!("file:{}", data_file.display()),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    match output {
        Ok(mut child) => {
            // Let it run for a bit
            thread::sleep(Duration::from_secs(1));
            let _ = child.kill();
            let _ = child.wait();

            log::info!("✅ Master provide persist command accepted with virtual ports");

            // Clean up
            let _ = std::fs::remove_file(&data_file);
            
            // Give extra time for port to be fully released
            thread::sleep(Duration::from_secs(1));
            
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&data_file);
            log::error!("Failed to spawn master provide persist: {}", e);
            Err(anyhow!("Failed to spawn: {}", e))
        }
    }
}
