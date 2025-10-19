/// Basic UI navigation and rendering tests
///
/// These tests verify that the TUI renders correctly and responds to keyboard input.
/// They check for visual elements like page titles, navigation hints, and UI structure.
use anyhow::Result;
use regex::Regex;

use ci_utils::{
    auto_cursor::{execute_cursor_actions, CursorAction},
    helpers::sleep_seconds,
    key_input::ArrowKey,
    snapshot::TerminalCapture,
    terminal::spawn_expect_process,
};

/// Test basic TUI navigation and page rendering
///
/// This test verifies:
/// 1. TUI starts and shows the Entry page
/// 2. Page title is visible
/// 3. Navigation to ConfigPanel works
/// 4. Edit mode shows brackets around editable values
/// 5. Escape returns to previous page
pub async fn test_ui_basic_navigation() -> Result<()> {
    log::info!("🧪 Starting basic UI navigation test");

    // Spawn TUI process
    log::info!("🚀 Spawning TUI process...");
    let mut tui_session = spawn_expect_process(&["--tui"])?;
    let mut tui_cap = TerminalCapture::new(24, 80);

    // Wait for TUI to initialize
    sleep_seconds(2).await;

    // Verify Entry page is shown
    log::info!("✅ Step 1: Verify Entry page is displayed");
    let actions = vec![
        CursorAction::Sleep { ms: 500 },
        CursorAction::MatchPattern {
            pattern: Regex::new(r"(?i)(entry|port.*list|serial.*port)")?,
            description: "Entry page title or port list visible".to_string(),
            line_range: Some((0, 5)),
            col_range: None,
            retry_action: None,
        },
    ];
    execute_cursor_actions(
        &mut tui_session,
        &mut tui_cap,
        &actions,
        "verify_entry_page",
    )
    .await?;

    // Test navigation: move cursor down if there are ports
    log::info!("✅ Step 2: Test cursor movement");
    let actions = vec![
        CursorAction::PressArrow {
            direction: ArrowKey::Down,
            count: 1,
        },
        CursorAction::Sleep { ms: 300 },
        CursorAction::PressArrow {
            direction: ArrowKey::Up,
            count: 1,
        },
        CursorAction::Sleep { ms: 300 },
    ];
    execute_cursor_actions(
        &mut tui_session,
        &mut tui_cap,
        &actions,
        "test_cursor_movement",
    )
    .await?;

    // Test page navigation with PageDown (if available)
    log::info!("✅ Step 3: Test PageDown/PageUp navigation");
    let actions = vec![
        CursorAction::PressPageDown,
        CursorAction::Sleep { ms: 500 },
        CursorAction::PressPageUp,
        CursorAction::Sleep { ms: 500 },
    ];
    execute_cursor_actions(
        &mut tui_session,
        &mut tui_cap,
        &actions,
        "test_page_navigation",
    )
    .await?;

    // Navigate into a port (if available) and check ConfigPanel
    log::info!("✅ Step 4: Test entering ConfigPanel");
    let screen = tui_cap.capture(&mut tui_session, "check_for_ports").await?;

    // Only proceed if there are ports available
    if screen.contains("/tmp") || screen.contains("/dev") || screen.contains("COM") {
        log::info!("   Ports found, testing ConfigPanel navigation");

        let actions = vec![
            CursorAction::PressEnter,
            CursorAction::Sleep { ms: 1000 },
            CursorAction::MatchPattern {
                pattern: Regex::new(r"(?i)(enable.*port|port.*config|baud.*rate)")?,
                description: "ConfigPanel with port settings visible".to_string(),
                line_range: Some((0, 20)),
                col_range: None,
                retry_action: None,
            },
        ];
        execute_cursor_actions(
            &mut tui_session,
            &mut tui_cap,
            &actions,
            "enter_config_panel",
        )
        .await?;

        // Test edit mode - check for brackets around editable values
        log::info!("✅ Step 5: Test edit mode visual feedback");
        let actions = vec![
            CursorAction::PressArrow {
                direction: ArrowKey::Down,
                count: 2,
            },
            CursorAction::Sleep { ms: 300 },
            CursorAction::PressEnter, // Enter edit mode
            CursorAction::Sleep { ms: 500 },
            CursorAction::MatchPattern {
                pattern: Regex::new(r"\[.*\]")?, // Check for brackets indicating edit mode
                description: "Edit mode brackets visible".to_string(),
                line_range: Some((0, 24)),
                col_range: None,
                retry_action: None,
            },
            CursorAction::PressEscape, // Exit edit mode
            CursorAction::Sleep { ms: 300 },
        ];
        execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "test_edit_mode").await?;

        // Navigate back to Entry page
        log::info!("✅ Step 6: Navigate back to Entry page");
        let actions = vec![
            CursorAction::PressEscape,
            CursorAction::Sleep { ms: 500 },
            CursorAction::MatchPattern {
                pattern: Regex::new(r"(?i)(entry|port.*list)")?,
                description: "Back to Entry page".to_string(),
                line_range: Some((0, 5)),
                col_range: None,
                retry_action: None,
            },
        ];
        execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "back_to_entry").await?;
    } else {
        log::info!("   No ports found, skipping ConfigPanel tests");
    }

    // Exit TUI
    log::info!("✅ Step 7: Exit TUI gracefully");
    let actions = vec![CursorAction::CtrlC, CursorAction::Sleep { ms: 500 }];
    execute_cursor_actions(&mut tui_session, &mut tui_cap, &actions, "exit_tui").await?;

    log::info!("🎉 Basic UI navigation test completed successfully!");

    Ok(())
}
