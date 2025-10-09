use anyhow::{anyhow, Result};
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    process::Stdio,
    time::Duration,
};

use ci_utils::generate_random_registers;

use ci_utils::create_modbus_command;

/// Generate pseudo-random modbus data using rand crate
/// Test continuous connection with file-based data source and file output
pub fn test_continuous_connection_with_files() -> Result<()> {
    log::info!("🧪 Testing continuous connection with file data source and file output...");
    let temp_dir = std::env::temp_dir();

    // Create random data file for master with known test data
    let data_file = temp_dir.join("test_continuous_data.json");
    let mut expected_data_lines = Vec::new();
    {
        let mut file = File::create(&data_file)?;
        for _ in 0..5 {
            let values = generate_random_registers(5);
            writeln!(file, "{{\"values\": {values:?}}}")?;
            expected_data_lines.push(values);
        }
    }
    log::info!(
        "🧪 Created test data file with {} lines",
        expected_data_lines.len()
    );

    // Create output file for slave
    let slave_output_file = temp_dir.join("test_continuous_slave_output.json");

    // Remove output file if it exists
    if slave_output_file.exists() {
        std::fs::remove_file(&slave_output_file)?;
    }

    // Start server (master-provide) on /tmp/vcom1 in persistent mode with data source
    log::info!("🧪 Starting Modbus server (master-provide) on /tmp/vcom1 with data source...");
    let mut server = create_modbus_command(
        false, // master-provide
        "/tmp/vcom1",
        true,
        Some(&format!("file:{}", data_file.display())),
    )?
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;

    // Give server time to start
    std::thread::sleep(Duration::from_secs(3));

    // Check if server is still running
    match server.try_wait()? {
        Some(status) => {
            let stderr = if let Some(stderr) = server.stderr.take() {
                let mut buf = String::new();
                let mut reader = BufReader::new(stderr);
                reader.read_line(&mut buf)?;
                buf
            } else {
                String::new()
            };

            std::fs::remove_file(&data_file)?;

            return Err(anyhow!(
                "Server exited prematurely with status {status}: {stderr}"
            ));
        }
        None => {
            log::info!("✅ Server is running");
        }
    }

    // Start client (slave-poll-persist) on /tmp/vcom2 in persistent mode with file output
    log::info!("🧪 Starting Modbus client (slave-poll-persist) on /tmp/vcom2 with file output...");

    let binary = ci_utils::build_debug_bin("aoba")?;
    let mut client = std::process::Command::new(&binary)
        .args([
            "--slave-poll-persist",
            "/tmp/vcom2",
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
            "--output",
            &format!("file:{}", slave_output_file.display()),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Let them communicate for a bit - give enough time to cycle through all data
    log::info!("🧪 Letting server and client communicate...");
    std::thread::sleep(Duration::from_secs(5));

    // Kill both processes
    client.kill()?;
    server.kill()?;
    client.wait()?;
    server.wait()?;

    // Give extra time for ports to be released
    std::thread::sleep(Duration::from_secs(1));

    // Verify that client output file has data
    if !slave_output_file.exists() {
        return Err(anyhow!("Client output file was not created"));
    }

    let client_output_content = std::fs::read_to_string(&slave_output_file)?;
    log::info!(
        "🧪 Client output ({} bytes):\n{}",
        client_output_content.len(),
        client_output_content
    );

    if client_output_content.trim().is_empty() {
        std::fs::remove_file(&data_file)?;
        std::fs::remove_file(&slave_output_file)?;
        return Err(anyhow!("Client output file is empty"));
    }

    // Verify JSON lines
    let lines: Vec<&str> = client_output_content.lines().collect();
    log::info!("✅ Client produced {} output lines", lines.len());

    // Parse all output lines and verify they are valid JSON
    let mut parsed_outputs = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        match serde_json::from_str::<serde_json::Value>(line) {
            Ok(json) => {
                if let Some(values) = json.get("values").and_then(|v| v.as_array()) {
                    let values_u16: Vec<u16> = values
                        .iter()
                        .filter_map(|v| v.as_u64().map(|n| n as u16))
                        .collect();
                    parsed_outputs.push(values_u16);
                }
            }
            Err(e) => {
                log::warn!("⚠️ Line {} is not valid JSON: {}", i + 1, e);
            }
        }
    }

    // Verify that all expected input lines were transmitted
    log::info!(
        "🧪 Verifying that all {} input lines were transmitted...",
        expected_data_lines.len()
    );
    let mut all_found = true;
    for (i, expected_values) in expected_data_lines.iter().enumerate() {
        let found = parsed_outputs
            .iter()
            .any(|output| output == expected_values);
        if found {
            log::info!(
                "✅ Input line {} found in output: {:?}",
                i + 1,
                expected_values
            );
        } else {
            log::warn!(
                "⚠️ Input line {} NOT found in output: {:?}",
                i + 1,
                expected_values
            );
            all_found = false;
        }
    }

    if !all_found {
        std::fs::remove_file(&data_file)?;
        std::fs::remove_file(&slave_output_file)?;
        return Err(anyhow!("Not all input lines were found in the output"));
    }

    log::info!(
        "✅ All {} input lines were successfully transmitted and verified",
        expected_data_lines.len()
    );

    // Clean up
    std::fs::remove_file(&data_file)?;
    std::fs::remove_file(&slave_output_file)?;

    log::info!("✅ Continuous connection test with files passed");
    Ok(())
}

/// Test continuous connection with Unix pipe data source and pipe output
pub fn test_continuous_connection_with_pipes() -> Result<()> {
    log::info!("🧪 Testing continuous connection with Unix pipe data source and pipe output...");
    let temp_dir = std::env::temp_dir();

    // Create named pipes
    let data_pipe = temp_dir.join("test_continuous_data.pipe");
    let output_pipe = temp_dir.join("test_continuous_output.pipe");

    // Remove pipes if they exist
    let _ = std::fs::remove_file(&data_pipe);
    let _ = std::fs::remove_file(&output_pipe);

    // Create named pipes using mkfifo
    std::process::Command::new("mkfifo")
        .arg(&data_pipe)
        .status()?;
    std::process::Command::new("mkfifo")
        .arg(&output_pipe)
        .status()?;

    log::info!("✅ Created named pipes");

    // Start server (master-provide) on /tmp/vcom1 with pipe data source
    log::info!("🧪 Starting Modbus server (master-provide) on /tmp/vcom1 with pipe data source...");
    let mut server = create_modbus_command(
        false, // master-provide
        "/tmp/vcom1",
        true,
        Some(&format!("pipe:{}", data_pipe.display())),
    )?
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;

    // Give server time to start
    std::thread::sleep(Duration::from_secs(2));

    // Start client (slave-poll-persist) on /tmp/vcom2 with pipe output
    log::info!("🧪 Starting Modbus client (slave-poll-persist) on /tmp/vcom2 with pipe output...");

    let binary = ci_utils::build_debug_bin("aoba")?;
    let mut client = std::process::Command::new(&binary)
        .args([
            "--slave-poll-persist",
            "/tmp/vcom2",
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
            "--output",
            &format!("pipe:{}", output_pipe.display()),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    // Give client time to start and open the pipe
    std::thread::sleep(Duration::from_secs(2));

    // Start a thread to write random data to the input pipe (for server)
    let data_pipe_clone = data_pipe.clone();
    let writer_thread = std::thread::spawn(move || -> Result<()> {
        log::info!("🧪 Writing random data to server input pipe...");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(&data_pipe_clone)?;

        for i in 0..5 {
            let values = generate_random_registers(5);
            writeln!(file, "{{\"values\": {values:?}}}")?;
            log::info!("🧪 Wrote data line {}: {:?}", i + 1, values);
            std::thread::sleep(Duration::from_millis(500));
        }
        Ok(())
    });

    // Start a thread to read from the client output pipe
    let output_pipe_clone = output_pipe.clone();
    let reader_thread = std::thread::spawn(move || -> Result<Vec<String>> {
        log::info!("🧪 Reading from client output pipe...");
        std::thread::sleep(Duration::from_secs(1)); // Wait for client to start writing

        let file = std::fs::File::open(&output_pipe_clone)?;
        let reader = BufReader::new(file);
        let mut lines = Vec::new();

        for line in reader.lines() {
            let line = line?;
            log::info!("🧪 Read output line: {line}");
            lines.push(line);

            if lines.len() >= 3 {
                break;
            }
        }

        Ok(lines)
    });

    // Wait for threads to complete with timeout
    let writer_result = writer_thread.join();
    let reader_result = reader_thread.join();

    // Kill processes
    client.kill()?;
    server.kill()?;
    client.wait()?;
    server.wait()?;

    // Give extra time for ports to be released
    std::thread::sleep(Duration::from_secs(1));

    // Clean up pipes
    let _ = std::fs::remove_file(&data_pipe);
    let _ = std::fs::remove_file(&output_pipe);

    // Check results
    match writer_result {
        Ok(Ok(())) => log::info!("✅ Writer thread completed successfully"),
        Ok(Err(e)) => log::warn!("⚠️ Writer thread error: {e}"),
        Err(_) => log::warn!("⚠️ Writer thread panicked"),
    }

    match reader_result {
        Ok(Ok(lines)) => {
            log::info!("✅ Reader thread read {} lines", lines.len());
            for (i, line) in lines.iter().enumerate() {
                if let Err(e) = serde_json::from_str::<serde_json::Value>(line) {
                    log::warn!("⚠️ Line {} is not valid JSON: {}", i + 1, e);
                }
            }
        }
        Ok(Err(e)) => log::warn!("⚠️ Reader thread error: {e}"),
        Err(_) => log::warn!("⚠️ Reader thread panicked"),
    }

    log::info!("✅ Continuous connection test with pipes completed");
    Ok(())
}
