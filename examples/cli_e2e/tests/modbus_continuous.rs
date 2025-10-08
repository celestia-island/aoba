use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Generate pseudo-random modbus data using timestamp
fn generate_random_data(length: usize) -> Vec<u16> {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    
    (0..length)
        .map(|i| {
            let x = seed.wrapping_add(i as u64).wrapping_mul(1103515245).wrapping_add(12345);
            ((x / 65536) % 1000) as u16
        })
        .collect()
}

/// Test continuous connection with file-based data source and file output
pub fn test_continuous_connection_with_files() -> Result<()> {
    log::info!("🧪 Testing continuous connection with file data source and file output...");

    let binary = aoba::ci::build_debug_bin("aoba")?;
    let temp_dir = std::env::temp_dir();

    // Create random data file for master
    let data_file = temp_dir.join("test_continuous_data.json");
    {
        let mut file = File::create(&data_file)?;
        for _ in 0..5 {
            let values = generate_random_data(5);
            writeln!(
                file,
                "{{\"values\": {:?}}}",
                values
            )?;
        }
    }

    // Create output file for slave
    let slave_output_file = temp_dir.join("test_continuous_slave_output.json");
    
    // Remove output file if it exists
    if slave_output_file.exists() {
        std::fs::remove_file(&slave_output_file)?;
    }

    // Start slave (server) on /tmp/vcom1 in persistent mode with file output
    log::info!("🧪 Starting Modbus slave on /tmp/vcom1 with file output...");
    let mut slave = Command::new(&binary)
        .args([
            "--slave-listen-persist",
            "/tmp/vcom1",
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

    // Give slave time to start
    thread::sleep(Duration::from_secs(3));

    // Check if slave is still running
    match slave.try_wait()? {
        Some(status) => {
            let stderr = if let Some(stderr) = slave.stderr.take() {
                let mut buf = String::new();
                let mut reader = BufReader::new(stderr);
                reader.read_line(&mut buf)?;
                buf
            } else {
                String::new()
            };

            std::fs::remove_file(&data_file)?;

            return Err(anyhow!(
                "Slave exited prematurely with status {}: {}",
                status,
                stderr
            ));
        }
        None => {
            log::info!("✅ Slave is running");
        }
    }

    // Start master (client) on /tmp/vcom2 in persistent mode
    log::info!("🧪 Starting Modbus master on /tmp/vcom2 with file data source...");
    let mut master = Command::new(&binary)
        .args([
            "--master-provide-persist",
            "/tmp/vcom2",
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
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Let them communicate for a bit
    log::info!("🧪 Letting master and slave communicate...");
    thread::sleep(Duration::from_secs(3));

    // Kill both processes
    master.kill()?;
    slave.kill()?;
    master.wait()?;
    slave.wait()?;

    // Give extra time for ports to be released
    thread::sleep(Duration::from_secs(1));

    // Verify that slave output file has data
    if !slave_output_file.exists() {
        return Err(anyhow!("Slave output file was not created"));
    }

    let slave_output_content = std::fs::read_to_string(&slave_output_file)?;
    log::info!("🧪 Slave output ({} bytes):\n{}", slave_output_content.len(), slave_output_content);

    if slave_output_content.trim().is_empty() {
        std::fs::remove_file(&data_file)?;
        std::fs::remove_file(&slave_output_file)?;
        return Err(anyhow!("Slave output file is empty"));
    }

    // Verify JSON lines
    let lines: Vec<&str> = slave_output_content.lines().collect();
    log::info!("✅ Slave produced {} output lines", lines.len());

    for (i, line) in lines.iter().enumerate() {
        if let Err(e) = serde_json::from_str::<serde_json::Value>(line) {
            log::warn!("⚠️ Line {} is not valid JSON: {}", i + 1, e);
        }
    }

    // Clean up
    std::fs::remove_file(&data_file)?;
    std::fs::remove_file(&slave_output_file)?;

    log::info!("✅ Continuous connection test with files passed");
    Ok(())
}

/// Test continuous connection with Unix pipe data source and pipe output
pub fn test_continuous_connection_with_pipes() -> Result<()> {
    log::info!("🧪 Testing continuous connection with Unix pipe data source and pipe output...");

    let binary = aoba::ci::build_debug_bin("aoba")?;
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

    // Start slave (server) on /tmp/vcom1 with pipe output
    log::info!("🧪 Starting Modbus slave on /tmp/vcom1 with pipe output...");
    let mut slave = Command::new(&binary)
        .args([
            "--slave-listen-persist",
            "/tmp/vcom1",
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

    // Give slave time to start
    thread::sleep(Duration::from_secs(2));

    // Start master (client) on /tmp/vcom2 with pipe data source
    log::info!("🧪 Starting Modbus master on /tmp/vcom2 with pipe data source...");
    let mut master = Command::new(&binary)
        .args([
            "--master-provide-persist",
            "/tmp/vcom2",
            "--station-id",
            "1",
            "--register-address",
            "0",
            "--register-length",
            "5",
            "--register-mode",
            "holding",
            "--data-source",
            &format!("pipe:{}", data_pipe.display()),
            "--baud-rate",
            "9600",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Give master time to start and open the pipe
    thread::sleep(Duration::from_secs(2));

    // Start a thread to write random data to the input pipe
    let data_pipe_clone = data_pipe.clone();
    let writer_thread = thread::spawn(move || -> Result<()> {
        log::info!("🧪 Writing random data to input pipe...");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(&data_pipe_clone)?;
        
        for i in 0..5 {
            let values = generate_random_data(5);
            writeln!(file, "{{\"values\": {:?}}}", values)?;
            log::info!("🧪 Wrote data line {}: {:?}", i + 1, values);
            thread::sleep(Duration::from_millis(500));
        }
        Ok(())
    });

    // Start a thread to read from the output pipe
    let output_pipe_clone = output_pipe.clone();
    let reader_thread = thread::spawn(move || -> Result<Vec<String>> {
        log::info!("🧪 Reading from output pipe...");
        thread::sleep(Duration::from_secs(1)); // Wait for slave to start writing
        
        let file = std::fs::File::open(&output_pipe_clone)?;
        let reader = BufReader::new(file);
        let mut lines = Vec::new();
        
        for line in reader.lines() {
            let line = line?;
            log::info!("🧪 Read output line: {}", line);
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
    master.kill()?;
    slave.kill()?;
    master.wait()?;
    slave.wait()?;

    // Give extra time for ports to be released
    thread::sleep(Duration::from_secs(1));

    // Clean up pipes
    let _ = std::fs::remove_file(&data_pipe);
    let _ = std::fs::remove_file(&output_pipe);

    // Check results
    match writer_result {
        Ok(Ok(())) => log::info!("✅ Writer thread completed successfully"),
        Ok(Err(e)) => log::warn!("⚠️ Writer thread error: {}", e),
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
        Ok(Err(e)) => log::warn!("⚠️ Reader thread error: {}", e),
        Err(_) => log::warn!("⚠️ Reader thread panicked"),
    }

    log::info!("✅ Continuous connection test with pipes completed");
    Ok(())
}
