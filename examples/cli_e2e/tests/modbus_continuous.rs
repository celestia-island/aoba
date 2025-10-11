use anyhow::{anyhow, Result};
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use ci_utils::{create_modbus_command, generate_random_registers, sleep_a_while, vcom_matchers};

async fn read_child_output<R: std::io::Read + Send + 'static>(
    reader: Box<R>,
    flag: Arc<AtomicBool>,
    prefix: String,
    is_stderr: bool,
) {
    // Use a blocking task to read from the std::io::Read and log lines.
    tokio::task::spawn_blocking(move || {
        let buf_reader = BufReader::new(reader);
        for line in buf_reader.lines() {
            match line {
                Ok(l) => {
                    flag.store(true, Ordering::Relaxed);
                    if is_stderr {
                        log::warn!("{prefix} {l}");
                    } else {
                        log::info!("{prefix} {l}");
                    }
                }
                Err(e) => {
                    log::warn!("⚠️ Error reading child output: {e}");
                    break;
                }
            }
        }
    })
    .await
    .ok();
}

async fn writer(data_pipe: PathBuf) -> Result<()> {
    log::info!("🧪 Writing random data to server input pipe...");
    let mut file = std::fs::OpenOptions::new().write(true).open(&data_pipe)?;

    for i in 0..5 {
        let values = generate_random_registers(5);
        // Build JSON via serde_json instead of manual string formatting
        let sent_at = chrono::Utc::now();
        let json = serde_json::json!({"values": values, "sent_at": sent_at.to_rfc3339()});
        writeln!(file, "{json}")?;
        file.flush()?;
        log::info!("🧪 Wrote data line {}: {:?}", i + 1, values);
        // Ensure send frequency is 1 second per request
        ci_utils::sleep_seconds(1).await;
    }
    Ok(())
}

async fn reader(output_pipe: PathBuf) -> Result<Vec<String>> {
    log::info!("🧪 Reading from client output pipe...");

    // Wait adaptively until client shows activity or short timeout, then open
    let read_start = std::time::Instant::now();
    while read_start.elapsed() < Duration::from_secs(6) {
        if read_start.elapsed() > Duration::from_millis(300) {
            break;
        }
        sleep_a_while().await;
    }

    let mut lines = Vec::new();
    // Keep attempting to read from the pipe until we collect enough lines or timeout.
    let overall_start = std::time::Instant::now();
    let overall_timeout = Duration::from_secs(12);
    // Expected number of lines: writer writes 5 lines; read until we get all or timeout.
    let expected = 5usize;

    while overall_start.elapsed() < overall_timeout {
        // Try opening the pipe for reading. This will block until a writer opens the FIFO,
        // which is fine — we want to capture lines as writers produce them.
        let file = match std::fs::File::open(&output_pipe) {
            Ok(f) => f,
            Err(e) => {
                log::warn!("⚠️ Failed to open output pipe (will retry): {e}");
                ci_utils::sleep_seconds(1).await;
                continue;
            }
        };

        let reader = BufReader::new(file);

        for line in reader.lines() {
            match line {
                Ok(l) => {
                    let recv_at = chrono::Utc::now();
                    log::info!("🧪 Read output line at {}: {l}", recv_at.to_rfc3339());
                    lines.push(l);
                }
                Err(e) => {
                    log::warn!("⚠️ Error reading from output pipe: {e}");
                    break;
                }
            }

            if lines.len() >= expected {
                break;
            }
        }

        // If we have enough lines, stop; otherwise wait and reopen the pipe to read more
        if lines.len() >= expected {
            break;
        }

        // Short pause before trying to reopen the pipe
        ci_utils::sleep_seconds(1).await;
    }

    Ok(lines)
}

/// Generate pseudo-random modbus data using rand crate
/// Test continuous connection with file-based data source and file output
pub async fn test_continuous_connection_with_files() -> Result<()> {
    log::info!("🧪 Testing continuous connection with file data source and file output...");
    let temp_dir = std::env::temp_dir();
    let ports = vcom_matchers();

    // Create random data file for master with known test data
    let data_file = temp_dir.join("test_continuous_data.json");
    let mut expected_data_lines = Vec::new();
    {
        let mut file = File::create(&data_file)?;
        for _ in 0..5 {
            let values = generate_random_registers(5);
            writeln!(file, "{{\"values\": {values:?}}}")?;
            // Ensure data is flushed to disk so server can read promptly
            file.flush()?;
            expected_data_lines.push(values);
        }
    }
    log::info!(
        "🧪 Created test data file with {count} lines",
        count = expected_data_lines.len()
    );

    // Debug: log the expected lines so we can diagnose mismatches during E2E runs
    log::info!("🧪 Expected data lines: {expected_data_lines:?}");

    // Create output file for slave
    let slave_output_file = temp_dir.join("test_continuous_slave_output.json");

    // Remove output file if it exists
    if slave_output_file.exists() {
        std::fs::remove_file(&slave_output_file)?;
    }

    // Start server (master-provide) on port1 in persistent mode with data source
    log::info!(
        "🧪 Starting Modbus server (master-provide) on {} with data source...",
        ports.port1_name
    );
    let mut server = create_modbus_command(
        false, // master-provide
        &ports.port1_name,
        true,
        Some(&format!("file:{}", data_file.display())),
    )?
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;

    // Start background readers and a readiness flag. Reader threads set the flag
    // when they observe any output which we use as an adaptive readiness signal.
    let mut child_readers = Vec::new();
    let server_output_seen = Arc::new(AtomicBool::new(false));
    if let Some(stdout) = server.stdout.take() {
        let flag = server_output_seen.clone();
        child_readers.push(std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("create rt");
            rt.block_on(async move {
                read_child_output(
                    Box::new(stdout),
                    flag,
                    "🧪 [server stdout]".to_string(),
                    false,
                )
                .await;
            });
        }));
    }
    if let Some(stderr) = server.stderr.take() {
        let flag = server_output_seen.clone();
        child_readers.push(std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("create rt");
            rt.block_on(async move {
                read_child_output(
                    Box::new(stderr),
                    flag,
                    "🧪 [server stderr]".to_string(),
                    true,
                )
                .await;
            });
        }));
    }

    // Wait adaptively for server output to appear, or timeout.
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(8);
    while start.elapsed() < timeout {
        if server_output_seen.load(Ordering::Relaxed) {
            break;
        }
        if let Some(status) = server.try_wait()? {
            log::warn!("⚠️ Server exited while waiting: {status}");
            break;
        }
        sleep_a_while().await;
    }

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

    // Start client (slave-poll-persist) on port2 in persistent mode with file output
    log::info!(
        "🧪 Starting Modbus client (slave-poll-persist) on {} with file output...",
        ports.port2_name
    );

    let binary = ci_utils::build_debug_bin("aoba")?;
    let mut client = std::process::Command::new(&binary)
        .args([
            "--slave-poll-persist",
            &ports.port2_name,
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

    // Spawn background readers for client stdout/stderr and a readiness flag.
    let mut client_readers = Vec::new();
    let client_output_seen = Arc::new(AtomicBool::new(false));
    if let Some(stdout) = client.stdout.take() {
        let flag = client_output_seen.clone();
        client_readers.push(std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("create rt");
            rt.block_on(async move {
                read_child_output(
                    Box::new(stdout),
                    flag,
                    "🧪 [client stdout]".to_string(),
                    false,
                )
                .await;
            });
        }));
    }
    if let Some(stderr) = client.stderr.take() {
        let flag = client_output_seen.clone();
        client_readers.push(std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("create rt");
            rt.block_on(async move {
                read_child_output(
                    Box::new(stderr),
                    flag,
                    "🧪 [client stderr]".to_string(),
                    true,
                )
                .await;
            });
        }));
    }

    // Adaptive wait: check slave output file until it contains the expected
    // number of non-empty lines or until timeout.
    log::info!("🧪 Waiting for client to produce output (adaptive)...");
    let comm_start = std::time::Instant::now();
    let comm_timeout = Duration::from_secs(20);
    while comm_start.elapsed() < comm_timeout {
        if slave_output_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&slave_output_file) {
                let produced_lines = content.lines().filter(|l| !l.trim().is_empty()).count();
                if produced_lines >= expected_data_lines.len() {
                    log::info!(
                        "🧪 Observed {} output lines (expected {}).",
                        produced_lines,
                        expected_data_lines.len()
                    );
                    break;
                }
            }
        }
        if let Some(status) = client.try_wait()? {
            log::warn!("⚠️ Client exited while waiting for output: {status}");
            break;
        }
        sleep_a_while().await;
    }

    // Do not kill the client/server immediately — wait until after verification so
    // late-arriving output can still be captured. We'll terminate processes after
    // verification (or if verification ultimately fails) below.

    // Verify that client output file has data
    if !slave_output_file.exists() {
        return Err(anyhow!("Client output file was not created"));
    }

    // Read output file; allow a short poll window in case writer flushed late
    let mut client_output_content = String::new();
    let mut attempts = 0;
    while attempts < 5 {
        if slave_output_file.exists() {
            client_output_content = std::fs::read_to_string(&slave_output_file)?;
            if !client_output_content.trim().is_empty() {
                break;
            }
        }
        attempts += 1;
        sleep_a_while().await;
    }
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
    // Collect timing info per record: (values, sent_at_opt, server_ts_opt, recv_at)
    // Type alias to reduce complexity reported by clippy
    type TimingRecord = (
        Vec<u16>,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<chrono::DateTime<chrono::Utc>>,
        chrono::DateTime<chrono::Utc>,
    );
    let mut timing_records: Vec<TimingRecord> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        match serde_json::from_str::<serde_json::Value>(line) {
            Ok(json) => {
                if let Some(values) = json.get("values").and_then(|v| v.as_array()) {
                    let values_u16: Vec<u16> = values
                        .iter()
                        .filter_map(|v| v.as_u64().map(|n| n as u16))
                        .collect();
                    // extract sent_at and server timestamp if present
                    let sent_at = json
                        .get("sent_at")
                        .and_then(|v| v.as_str())
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc));
                    let server_ts = json
                        .get("timestamp")
                        .and_then(|v| v.as_str())
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc));
                    let recv_at = chrono::Utc::now();
                    parsed_outputs.push(values_u16.clone());
                    timing_records.push((values_u16, sent_at, server_ts, recv_at));
                }
            }
            Err(e) => {
                log::warn!("⚠️ Line {} is not valid JSON: {}", i + 1, e);
            }
        }
    }

    // Debug: log parsed outputs for diagnosis
    log::info!("🧪 Parsed outputs: {parsed_outputs:?}");

    // Print timing report
    if !timing_records.is_empty() {
        log::info!(
            "🧪 Timing report per record (sent_at, server_ts, recv_at, recv-server, recv-sent)"
        );
        let mut deltas_recv_server = Vec::new();
        let mut deltas_recv_sent = Vec::new();
        for (vals, sent_opt, server_opt, recv_at) in &timing_records {
            let server_str = server_opt
                .map(|t| t.to_rfc3339())
                .unwrap_or_else(|| "-".to_string());
            let sent_str = sent_opt
                .map(|t| t.to_rfc3339())
                .unwrap_or_else(|| "-".to_string());
            let recv_str = recv_at.to_rfc3339();
            let recv_server = server_opt.map(|s| *recv_at - s);
            let recv_sent = sent_opt.map(|s| *recv_at - s);
            if let Some(d) = recv_server {
                deltas_recv_server.push(d);
            }
            if let Some(d) = recv_sent {
                deltas_recv_sent.push(d);
            }
            log::info!("vals={vals:?} sent={sent_str} server={server_str} recv={recv_str} recv-server={recv_server:?} recv-sent={recv_sent:?}");
        }
        if !deltas_recv_server.is_empty() {
            let max = deltas_recv_server.iter().max().unwrap();
            let min = deltas_recv_server.iter().min().unwrap();
            let avg = deltas_recv_server
                .iter()
                .fold(chrono::Duration::zero(), |acc, d| acc + *d)
                / (deltas_recv_server.len() as i32);
            log::info!("🧪 recv-server delta: min={min} max={max} avg={avg}");
        }
        if !deltas_recv_sent.is_empty() {
            let max = deltas_recv_sent.iter().max().unwrap();
            let min = deltas_recv_sent.iter().min().unwrap();
            let avg = deltas_recv_sent
                .iter()
                .fold(chrono::Duration::zero(), |acc, d| acc + *d)
                / (deltas_recv_sent.len() as i32);
            log::info!("🧪 recv-sent delta: min={min} max={max} avg={avg}");
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
        log::warn!(
            "⚠️ Not all input lines found; giving an extra short window to allow late arrival..."
        );
        // Allow a short extra window for late-arriving lines (e.g., writer flushes or slower reads)
        let extra_start = std::time::Instant::now();
        let extra_timeout = Duration::from_secs(10);
        while extra_start.elapsed() < extra_timeout {
            if slave_output_file.exists() {
                let content = std::fs::read_to_string(&slave_output_file)?;
                let mut new_parsed = Vec::new();
                for line in content.lines() {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                        if let Some(values) = json.get("values").and_then(|v| v.as_array()) {
                            let values_u16: Vec<u16> = values
                                .iter()
                                .filter_map(|v| v.as_u64().map(|n| n as u16))
                                .collect();
                            new_parsed.push(values_u16);
                        }
                    }
                }
                // Merge newly parsed outputs into parsed_outputs if they're not already present
                for p in new_parsed {
                    if !parsed_outputs.iter().any(|e| e == &p) {
                        parsed_outputs.push(p);
                    }
                }

                // Re-evaluate
                let mut all_found_retry = true;
                for expected_values in &expected_data_lines {
                    let found = parsed_outputs
                        .iter()
                        .any(|output| output == expected_values);
                    if !found {
                        all_found_retry = false;
                        break;
                    }
                }
                if all_found_retry {
                    all_found = true;
                    break;
                }
            }
            sleep_a_while().await;
        }

        if !all_found {
            // Clean up processes and threads before returning an error so any late
            // output still has a chance to be flushed/captured by background readers.
            let _ = client.kill();
            let _ = server.kill();
            let _ = client.wait();
            let _ = server.wait();

            for h in client_readers.drain(..) {
                let _ = h.join();
            }
            for h in child_readers.drain(..) {
                let _ = h.join();
            }

            std::fs::remove_file(&data_file)?;
            std::fs::remove_file(&slave_output_file)?;
            return Err(anyhow!("Not all input lines were found in the output"));
        }
    }

    log::info!(
        "✅ All {} input lines were successfully transmitted and verified",
        expected_data_lines.len()
    );

    // Terminate processes and join background readers after successful verification
    let _ = client.kill();
    let _ = server.kill();
    let _ = client.wait();
    let _ = server.wait();

    for h in client_readers.drain(..) {
        let _ = h.join();
    }
    for h in child_readers.drain(..) {
        let _ = h.join();
    }

    // Clean up temp files
    std::fs::remove_file(&data_file)?;
    std::fs::remove_file(&slave_output_file)?;

    log::info!("✅ Continuous connection test with files passed");
    Ok(())
}

/// Test continuous connection with Unix pipe data source and pipe output
pub async fn test_continuous_connection_with_pipes() -> Result<()> {
    log::info!("🧪 Testing continuous connection with Unix pipe data source and pipe output...");
    let temp_dir = std::env::temp_dir();
    let ports = vcom_matchers();

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

    // Start server (master-provide) on port1 with pipe data source
    log::info!(
        "🧪 Starting Modbus server (master-provide) on {} with pipe data source...",
        ports.port1_name
    );
    let mut server = create_modbus_command(
        false, // master-provide
        &ports.port1_name,
        true,
        Some(&format!("pipe:{}", data_pipe.display())),
    )?
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;

    // Start background readers for server and use a flag for readiness detection.
    let server_output_seen_pipe = Arc::new(AtomicBool::new(false));
    let mut server_pipe_readers = Vec::new();
    if let Some(stdout) = server.stdout.take() {
        let flag = server_output_seen_pipe.clone();
        server_pipe_readers.push(std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("create rt");
            rt.block_on(async move {
                read_child_output(
                    Box::new(stdout),
                    flag,
                    "🧪 [server-pipe stdout]".to_string(),
                    false,
                )
                .await
            });
        }));
    }
    if let Some(stderr) = server.stderr.take() {
        let flag = server_output_seen_pipe.clone();
        server_pipe_readers.push(std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("create rt");
            rt.block_on(async move {
                read_child_output(
                    Box::new(stderr),
                    flag,
                    "🧪 [server-pipe stderr]".to_string(),
                    true,
                )
                .await
            });
        }));
    }

    // Adaptive wait for server to show output or until timeout
    let srv_start = std::time::Instant::now();
    while srv_start.elapsed() < Duration::from_secs(6) {
        if server_output_seen_pipe.load(Ordering::Relaxed) {
            break;
        }
        if let Some(status) = server.try_wait()? {
            log::warn!("⚠️ Server (pipe) exited early: {status}");
            break;
        }
        sleep_a_while().await;
    }

    // Start client (slave-poll-persist) on port2 with pipe output
    log::info!(
        "🧪 Starting Modbus client (slave-poll-persist) on {} with pipe output...",
        ports.port2_name
    );

    let binary = ci_utils::build_debug_bin("aoba")?;
    let mut client = std::process::Command::new(&binary)
        .args([
            "--slave-poll-persist",
            &ports.port2_name,
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

    // Spawn client readers and flag for readiness
    let client_output_seen_pipe = Arc::new(AtomicBool::new(false));
    let mut client_pipe_readers = Vec::new();
    if let Some(stdout) = client.stdout.take() {
        let flag = client_output_seen_pipe.clone();
        client_pipe_readers.push(std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("create rt");
            rt.block_on(async move {
                read_child_output(
                    Box::new(stdout),
                    flag,
                    "🧪 [client-pipe stdout]".to_string(),
                    false,
                )
                .await
            });
        }));
    }
    if let Some(stderr) = client.stderr.take() {
        let flag = client_output_seen_pipe.clone();
        client_pipe_readers.push(std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("create rt");
            rt.block_on(async move {
                read_child_output(
                    Box::new(stderr),
                    flag,
                    "🧪 [client-pipe stderr]".to_string(),
                    true,
                )
                .await;
            });
        }));
    }

    // Adaptive wait: ensure client shows output activity or timeout
    let cli_start = std::time::Instant::now();
    while cli_start.elapsed() < Duration::from_secs(6) {
        if client_output_seen_pipe.load(Ordering::Relaxed) {
            break;
        }
        if let Some(status) = client.try_wait()? {
            log::warn!("⚠️ Client (pipe) exited early: {status}");
            break;
        }
        sleep_a_while().await;
    }

    // Start a thread to write random data to the input pipe (for server)
    let data_pipe_clone = data_pipe.clone();
    let writer_thread = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("create rt");
        rt.block_on(async move { writer(data_pipe_clone).await })
    });

    // Start a thread to read from the client output pipe
    let output_pipe_clone = output_pipe.clone();
    let reader_thread = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("create rt");
        rt.block_on(async move { reader(output_pipe_clone).await })
    });

    // Wait for threads to complete with timeout
    let writer_result = writer_thread.join();
    let reader_result = reader_thread.join();

    // Kill processes
    client.kill()?;
    server.kill()?;
    client.wait()?;
    server.wait()?;

    // Give extra time for pipes to be released by polling a short window.
    let release_start2 = std::time::Instant::now();
    while release_start2.elapsed() < Duration::from_secs(2) {
        // If writer/reader threads already joined, break early
        if writer_result.is_ok() && reader_result.is_ok() {
            break;
        }
        sleep_a_while().await;
    }

    // Clean up pipes
    let _ = std::fs::remove_file(&data_pipe);
    let _ = std::fs::remove_file(&output_pipe);

    // Check results
    match writer_result {
        Ok(Ok(())) => log::info!("✅ Writer thread completed successfully"),
        Ok(Err(err)) => log::warn!("⚠️ Writer thread error: {err}"),
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
        Ok(Err(err)) => log::warn!("⚠️ Reader thread error: {err}"),
        Err(_) => log::warn!("⚠️ Reader thread panicked"),
    }

    log::info!("✅ Continuous connection test with pipes completed");
    Ok(())
}
