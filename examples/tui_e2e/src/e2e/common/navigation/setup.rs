use anyhow::{anyhow, Result};

use crate::e2e::common::state_helpers::{create_config_panel_state, create_entry_state};
use crate::e2e::common::state_updaters;
use aoba_ci_utils::*;

/// Setup TUI test environment with initialized session and terminal capture.
///
/// # Purpose
///
/// This is the **primary initialization function** for all TUI E2E tests. It:
/// 1. Validates serial port availability
/// 2. Spawns the TUI process in debug CI mode **with `--no-config-cache`**
/// 3. Waits for TUI initialization (3 seconds + page detection)
/// 4. Navigates from Entry page to ConfigPanel
/// 5. Returns ready-to-use session and capture objects
///
/// # Configuration Cache Handling
///
/// TUI is started with `--no-config-cache` flag, which disables loading and saving
/// of `aoba_tui_config.json`. This ensures each test starts with a completely clean
/// state without interference from previous test runs. No manual cache cleanup is needed.
///
/// # Parameters
///
/// - `port1`: Primary serial port name (e.g., "COM3", "/dev/ttyUSB0")
///   - Must exist and be accessible
///   - Used for main Modbus operations in tests
/// - `_port2`: Secondary port (currently unused, reserved for future multi-port tests)
///   - Prefix `_` indicates intentional non-use
///
/// # Returns
///
/// - `Ok((session, capture))`: Tuple of initialized TUI session and terminal capture
///   - `session`: `impl Expect` - Expectrl session for sending commands and reading output
///   - `capture`: `TerminalCapture` - Screen capture tool configured with Small size (80x24)
/// - `Err`: Port doesn't exist, TUI spawn failed, or initialization timeout
///
/// # Timing Behavior
///
/// - **TUI Spawn**: Immediate
/// - **Initial Wait**: 3 seconds (hard-coded for TUI startup)
/// - **Entry Page Wait**: Up to 10 seconds (via `wait_for_tui_page`)
/// - **ConfigPanel Navigation**: 1 second sleep after Enter key
/// - **ConfigPanel Wait**: Up to 10 seconds (via `wait_for_tui_page`)
/// - **Total Duration**: ~5-15 seconds depending on system performance
///
/// # Error Handling
///
/// This function can fail at several stages:
///
/// - **Port Validation**: `"Port {port1} does not exist"`
///   - Check port name is correct and device is connected
///   - Use `list_ports()` CLI command to verify available ports
///
/// - **TUI Spawn Failure**: `spawn_expect_process` error
///   - Verify AOBA binary is built and in PATH
///   - Check permissions for terminal access
///
/// - **Entry Page Timeout**: `wait_for_tui_page` timeout after 10 seconds
///   - TUI may be stuck or slow to start
///   - Check system resources (CPU, memory)
///   - Review TUI logs for startup errors
///
/// - **ConfigPanel Navigation**: Unexpected screen state
///   - TUI may have shown error dialog or unexpected page
///   - Capture screenshot to debug navigation state
///
/// # Debug Tips
///
/// - Verify AOBA is built and accessible with `cargo build --release`
/// - Check for port conflicts using `lsof` (Unix) or `mode` (Windows)
/// - Capture the current screen via [`TerminalCapture::capture`] if setup stalls
pub async fn setup_tui_test(
    port1: &str,
    _port2: &str,
    screenshot_ctx: Option<&ScreenshotContext>,
) -> Result<(impl ExpectSession, TerminalCapture)> {
    log::info!("🔧 Setting up TUI test environment for port {port1}");

    // Check if we're in generation mode
    let is_generation_mode = screenshot_ctx
        .map(|ctx| ctx.mode() == ExecutionMode::GenerateScreenshots)
        .unwrap_or(false);

    if !port_exists(port1) && !is_generation_mode {
        return Err(anyhow!("Port {port1} does not exist"));
    }

    // Start TUI session (even in generation mode, but we won't interact with it much)
    log::info!("Starting TUI in debug mode with --no-config-cache...");
    // Use Large terminal size to match screenshot generation (40 rows x 80 columns)
    let terminal_size = TerminalSize::Large;
    let (rows, cols) = terminal_size.dimensions();
    let mut tui_session = spawn_expect_session_with_size(
        &["--tui", "--debug-ci-e2e-test", "--no-config-cache"],
        Some((rows, cols)),
    )?;
    let mut tui_cap = TerminalCapture::with_size(terminal_size);

    // Only wait for pages in normal mode
    if !is_generation_mode {
        sleep_3s().await;
        log::info!("Waiting for TUI Entry page...");
        wait_for_tui_page("Entry", 10, None).await?;
    }

    // Generate/verify entry screenshot
    if let Some(ctx) = screenshot_ctx {
        let mut state = create_entry_state();
        state_updaters::add_discovered_ports(&mut state, port1, _port2);
        let entry_filename = ctx
            .capture_or_verify(&mut tui_session, &mut tui_cap, state, "entry")
            .await?;

        if !is_generation_mode {
            let entry_step_name = entry_filename.trim_end_matches(".txt").to_string();
            TuiStep::new("verify_entry_screen")
                .with_assertions(vec![ScreenAssertion::capture(ScreenCaptureSpec::new(
                    ctx.module_path().to_string(),
                    entry_step_name,
                    "Entry screen matches reference",
                ))])
                .run(&mut tui_session, &mut tui_cap)
                .await?;
        }
    }

    // Navigate to ConfigPanel (only in normal mode)
    if !is_generation_mode {
        log::info!("Navigating to ConfigPanel...");
        let actions = vec![CursorAction::PressEnter, CursorAction::Sleep1s];
        execute_cursor_actions(
            &mut tui_session,
            &mut tui_cap,
            &actions,
            "enter_config_panel",
        )
        .await?;

        wait_for_tui_page("ConfigPanel", 10, None).await?;
    }

    // Generate/verify config_panel screenshot
    if let Some(ctx) = screenshot_ctx {
        let mut config_state = create_config_panel_state(port1);
        // Add the second discovered port
        config_state.ports.push(TuiPort {
            name: _port2.to_string(),
            enabled: false,
            state: E2EPortState::Free,
            modbus_masters: Vec::new(),
            modbus_slaves: Vec::new(),
            log_count: 0,
        });
        let config_filename = ctx
            .capture_or_verify(&mut tui_session, &mut tui_cap, config_state, "config_panel")
            .await?;

        if !is_generation_mode {
            let step_name = config_filename.trim_end_matches(".txt").to_string();
            TuiStep::new("verify_config_panel")
                .with_assertions(vec![ScreenAssertion::capture(ScreenCaptureSpec::new(
                    ctx.module_path().to_string(),
                    step_name,
                    "Config panel matches reference",
                ))])
                .run(&mut tui_session, &mut tui_cap)
                .await?;
        }
    }

    log::info!("✅ TUI test environment ready");
    Ok((tui_session, tui_cap))
}
