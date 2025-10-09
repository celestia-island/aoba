// Test TUI Master (Slave/Server) with CLI Slave (Master/Client) - Continuous mode
// This test performs continuous random data updates in TUI Master and verifies CLI Slave receives them correctly
// Tests all 4 register types: holding, input, coils, discrete

use anyhow::{anyhow, Result};
use regex::Regex;
use std::{
    process::{Command, Stdio},
    time::Duration,
};

use expectrl::Expect;

use ci_utils::{
    auto_cursor::{execute_cursor_actions, CursorAction},
    data::{generate_random_coils, generate_random_registers},
    tui::{enable_port_carefully, navigate_to_vcom, update_tui_registers},
    verify::verify_continuous_data,
    {should_run_vcom_tests, sleep_a_while, spawn_expect_process, TerminalCapture},
};

/// Test TUI Master with CLI Slave - Continuous mode
/// This test runs continuous random updates and verifies data integrity
pub async fn test_tui_master_continuous_with_cli_slave(register_mode: &str) -> Result<()> {
    if !should_run_vcom_tests() {
        log::info!("Skipping TUI Master continuous test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting TUI Master + CLI Slave continuous test (mode: {register_mode})");

    // Verify vcom ports exist
    if !std::path::Path::new("/tmp/vcom1").exists() {
        return Err(anyhow!("/tmp/vcom1 was not created by socat"));
    }
    if !std::path::Path::new("/tmp/vcom2").exists() {
        return Err(anyhow!("/tmp/vcom2 was not created by socat"));
    }
    log::info!("✓ /tmp/vcom1 and /tmp/vcom2 verified");

    // Determine if this is a coil type register
    let is_coil = register_mode == "coils" || register_mode == "discrete";
    let register_length = if is_coil { 8 } else { 6 };

    // Spawn TUI process (will be master on vcom1)
    log::info!("🧪 Step 1: Spawning TUI process");
    let mut tui_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn TUI process: {err}"))?;
    let mut tui_cap = TerminalCapture::new(24, 80);

    sleep_a_while().await;

    // Wait for initial screen and verify TUI loaded
    log::info!("🧪 Step 2: Verify TUI loaded");
    let actions = vec![CursorAction::MatchPattern {
        pattern: Regex::new(r"AOBA")?,
        description: "TUI application title visible".to_string(),
        line_range: Some((0, 3)),
        col_range: None,
    }];
    execute_cursor_actions(
        &mut tui_session,
        &mut tui_cap,
        &actions,
        "verify_tui_loaded",
    )
    .await?;

    // Navigate to vcom1
    log::info!("🧪 Step 3: Navigate to vcom1");
    navigate_to_vcom(&mut tui_session, &mut tui_cap).await?;

    // Enable the port FIRST (before configuration)
    log::info!("🧪 Step 4: Enable the port");
    enable_port_carefully(&mut tui_session, &mut tui_cap).await?;

    // Configure TUI as Master with initial values (AFTER enabling port)
    log::info!("🧪 Step 5: Configure TUI as Master (mode: {register_mode})");
    let initial_values = if is_coil {
        generate_random_coils(register_length)
    } else {
        generate_random_registers(register_length)
    };
    log::info!("Initial values: {initial_values:?}");
    configure_tui_master(
        &mut tui_session,
        &mut tui_cap,
        register_mode,
        register_length,
        &initial_values,
    )
    .await?;

    // Wait for port initialization and hot reload to complete
    log::info!("🧪 Step 6: Wait for Modbus daemon to initialize and load configuration");
    // Need to wait longer for the Modbus daemon to actually start and reload config
    tokio::time::sleep(Duration::from_secs(5)).await;
    log::info!("  Waited 5 seconds for daemon initialization and config reload");

    // Verify TUI master is responding before starting persistent polling
    log::info!("🧪 Step 6.5: Verify TUI master is responding");
    let binary = ci_utils::build_debug_bin("aoba")?;

    // Retry the poll multiple times to give the daemon time to fully initialize
    let mut last_error = String::new();
    let mut test_poll_result = None;
    for attempt in 1..=5 {
        log::info!("  Poll attempt {attempt}/5");
        let test_poll = Command::new(&binary)
            .args([
                "--slave-poll",
                "/tmp/vcom2",
                "--baud-rate",
                "9600",
                "--station-id",
                "1",
                "--register-mode",
                register_mode,
                "--register-address",
                "0",
                "--register-length",
                &register_length.to_string(),
                "--json",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        if test_poll.status.success() {
            test_poll_result = Some(test_poll);
            log::info!("  ✓ TUI master is responding!");
            break;
        } else {
            last_error = String::from_utf8_lossy(&test_poll.stderr).to_string();
            log::warn!("  Attempt {attempt} failed: {last_error}");
            if attempt < 5 {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }

    let test_poll = test_poll_result.ok_or_else(|| {
        anyhow!("TUI master is not responding after 5 attempts. Last error: {last_error}")
    })?;

    let test_output = String::from_utf8_lossy(&test_poll.stdout);
    log::info!(
        "✅ TUI master responding, test poll output: {}",
        test_output.trim()
    );

    // Prepare output file for CLI slave
    let temp_dir = std::env::temp_dir();
    let output_file = temp_dir.join(format!("tui_master_continuous_{register_mode}.json"));
    if output_file.exists() {
        std::fs::remove_file(&output_file)?;
    }

    // Start CLI slave in persistent mode to continuously poll
    log::info!("🧪 Step 7: Start CLI slave in persistent mode");
    let mut cli_slave = Command::new(&binary)
        .args([
            "--slave-poll-persist",
            "/tmp/vcom2",
            "--baud-rate",
            "9600",
            "--station-id",
            "1",
            "--register-mode",
            register_mode,
            "--register-address",
            "0",
            "--register-length",
            &register_length.to_string(),
            "--output",
            &format!("file:{}", output_file.display()),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Give CLI slave time to start
    sleep_a_while().await;

    // Check if CLI slave is still running
    match cli_slave.try_wait()? {
        Some(status) => {
            // CLI slave exited - capture stderr for debugging
            let stderr = if let Some(mut stderr_handle) = cli_slave.stderr.take() {
                let mut buf = String::new();
                use std::io::Read;
                stderr_handle.read_to_string(&mut buf).ok();
                buf
            } else {
                String::new()
            };
            return Err(anyhow!(
                "CLI slave exited prematurely with status {status}, stderr: {stderr}"
            ));
        }
        None => {
            log::info!("✅ CLI slave is running");
        }
    }

    // Perform continuous random updates (3 iterations)
    let mut all_expected_values = vec![initial_values.clone()];
    log::info!("🧪 Step 8: Perform continuous random updates");

    for iteration in 0..3 {
        log::info!("--- Iteration {} ---", iteration + 1);

        // Wait a bit for previous values to be polled
        sleep_a_while().await;

        // Generate new random values
        let new_values = if is_coil {
            generate_random_coils(register_length)
        } else {
            generate_random_registers(register_length)
        };
        log::info!("New values (iteration {}): {:?}", iteration + 1, new_values);
        all_expected_values.push(new_values.clone());

        // Update registers in TUI
        update_tui_registers(&mut tui_session, &mut tui_cap, &new_values, is_coil).await?;

        log::info!("✓ Updated registers in TUI");
    }

    // Wait for final values to be polled
    sleep_a_while().await;

    // Check if output file was created
    while !output_file.exists() {
        log::warn!("⚠️ Output file doesn't exist yet, waiting longer...");
        sleep_a_while().await;
    }

    // Stop CLI slave
    log::info!("🧪 Step 9: Stop CLI slave");
    cli_slave.kill()?;
    cli_slave.wait()?;

    // Verify collected data from CLI output
    log::info!("🧪 Step 10: Verify collected data from CLI output");

    // Check if file exists and has content
    if !output_file.exists() {
        return Err(anyhow!(
            "Output file does not exist: {}. CLI slave may not have successfully polled any data.",
            output_file.display()
        ));
    }

    let file_size = std::fs::metadata(&output_file)?.len();
    if file_size == 0 {
        return Err(anyhow!(
            "Output file is empty: {}. CLI slave may not have received responses from TUI master.",
            output_file.display()
        ));
    }

    log::info!("Output file exists with {file_size} bytes");
    verify_continuous_data(&output_file, &all_expected_values, is_coil)?;

    // Capture screen to verify TUI display consistency
    log::info!("🧪 Step 11: Capture screen to verify TUI display");
    let screen = tui_cap.capture(&mut tui_session, "final_screen")?;
    log::info!("📸 Final screen captured");

    // Verify screen shows register values
    let has_values = screen.contains("0x") && !screen.lines().all(|l| l.contains("0x0000"));
    if has_values {
        log::info!("✅ TUI screen shows register values (hex patterns found)");
    } else {
        log::warn!("⚠️ TUI screen may not show expected values");
    }

    // Cleanup
    log::info!("🧪 Step 12: Cleanup");
    let quit_actions = vec![CursorAction::CtrlC];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &quit_actions, "quit_tui").await?;

    // Clean up output file
    if output_file.exists() {
        std::fs::remove_file(&output_file)?;
    }

    log::info!(
        "✅ TUI Master + CLI Slave continuous test completed successfully (mode: {register_mode})"
    );
    Ok(())
}

/// Configure TUI as Modbus Master with initial values
async fn configure_tui_master<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    register_mode: &str,
    register_length: usize,
    initial_values: &[u16],
) -> Result<()> {
    log::info!("📝 Configuring as Master (mode: {register_mode})...");

    // Navigate to Business Configuration
    let actions = vec![
        CursorAction::PressArrow {
            direction: ci_utils::ArrowKey::Down,
            count: 2,
        },
        CursorAction::Sleep { ms: 500 },
        CursorAction::PressEnter,
        CursorAction::MatchPattern {
            pattern: Regex::new(r"ModBus Master/Slave Settings")?,
            description: "In Modbus settings".to_string(),
            line_range: Some((0, 3)),
            col_range: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "enter_business_config").await?;

    // Create station
    let actions = vec![
        CursorAction::PressEnter,
        CursorAction::MatchPattern {
            pattern: Regex::new(r"#1")?,
            description: "Station #1 created".to_string(),
            line_range: None,
            col_range: None,
        },
    ];
    execute_cursor_actions(session, cap, &actions, "create_station").await?;

    // Navigate to Register Mode and set it
    log::info!("Setting register mode to: {register_mode}");
    let actions = vec![
        CursorAction::PressArrow {
            direction: ci_utils::ArrowKey::Down,
            count: 3,
        },
        CursorAction::PressEnter,
    ];
    execute_cursor_actions(session, cap, &actions, "nav_to_reg_mode").await?;

    // Select the appropriate register mode
    let arrow_count = match register_mode {
        "holding" => 0,
        "input" => 1,
        "coils" => 2,
        "discrete" => 3,
        _ => return Err(anyhow!("Invalid register mode: {register_mode}")),
    };

    if arrow_count > 0 {
        let actions = vec![CursorAction::PressArrow {
            direction: ci_utils::ArrowKey::Right,
            count: arrow_count,
        }];
        execute_cursor_actions(session, cap, &actions, "select_reg_mode").await?;
    }

    let actions = vec![CursorAction::PressEnter];
    execute_cursor_actions(session, cap, &actions, "confirm_reg_mode").await?;

    // Navigate to Register Length and set it
    log::info!("Setting register length to: {register_length}");
    let actions = vec![
        CursorAction::PressArrow {
            direction: ci_utils::ArrowKey::Down,
            count: 2,
        },
        CursorAction::PressEnter,
        CursorAction::TypeString(register_length.to_string()),
        CursorAction::PressEnter,
    ];
    execute_cursor_actions(session, cap, &actions, "set_reg_length").await?;

    // Navigate to register values
    let actions = vec![CursorAction::PressArrow {
        direction: ci_utils::ArrowKey::Down,
        count: 1,
    }];
    execute_cursor_actions(session, cap, &actions, "nav_to_registers").await?;

    // Set initial register values
    log::info!("Setting initial register values: {initial_values:?}");
    for (i, &val) in initial_values.iter().enumerate() {
        let dec_val = format!("{val}"); // Format as decimal, not hex
        let actions = vec![
            CursorAction::PressEnter,
            CursorAction::TypeString(dec_val),
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 500 }, // Wait for value to be saved
        ];
        execute_cursor_actions(session, cap, &actions, &format!("set_reg_{i}")).await?;

        if i < initial_values.len() - 1 {
            let actions = vec![CursorAction::PressArrow {
                direction: ci_utils::ArrowKey::Right,
                count: 1,
            }];
            execute_cursor_actions(session, cap, &actions, &format!("nav_to_reg_{}", i + 1))
                .await?;
        }
    }

    // After configuration, restart the port to ensure daemon loads the config
    // Exit the Modbus panel first
    log::info!("✓ Master configuration complete");
    log::info!("  Exiting Modbus panel to restart port...");

    let actions = vec![
        CursorAction::PressEscape,
        CursorAction::Sleep { ms: 500 },
        CursorAction::PressEscape,
        CursorAction::Sleep { ms: 1000 },
    ];
    execute_cursor_actions(session, cap, &actions, "exit_modbus_panel").await?;

    // Check where we are and navigate to port details if needed
    let screen = cap.capture(session, "after_exit")?;
    if screen.contains("COM Ports") {
        // We're at port list, need to enter vcom1
        log::info!("  Re-entering vcom1 port");
        let actions = vec![CursorAction::PressEnter, CursorAction::Sleep { ms: 500 }];
        execute_cursor_actions(session, cap, &actions, "enter_vcom1").await?;
    }

    // Now toggle the port OFF (disable)
    log::info!("  Toggling port OFF to apply configuration");
    let actions = vec![CursorAction::PressEnter, CursorAction::Sleep { ms: 1000 }];
    execute_cursor_actions(session, cap, &actions, "disable_port").await?;

    // Toggle the port back ON (enable)
    log::info!("  Toggling port ON with new configuration");
    let actions = vec![CursorAction::PressEnter, CursorAction::Sleep { ms: 1500 }];
    execute_cursor_actions(session, cap, &actions, "re_enable_port").await?;

    log::info!("✓ Port restarted with configuration");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("🧪 Running TUI E2E Continuous Test: TUI Master + CLI Slave (holding registers)");

    test_tui_master_continuous_with_cli_slave("holding").await?;

    log::info!("\n✅ TUI Master continuous test passed for holding registers!");
    Ok(())
}
