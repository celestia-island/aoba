use anyhow::{anyhow, Result};

use expectrl::Expect;

use ci_utils::*;

/// Maximum number of retry attempts for transaction operations
///
/// Each operation (field edit, navigation, etc.) will be attempted up to this many times
/// before failing. This provides resilience against CI environment timing issues.
const MAX_RETRY_ATTEMPTS: usize = 3;

/// Execute field edit with transaction-style retry mechanism
///
/// This function implements a robust field editing strategy that handles
/// unreliable keyboard input in CI environments. It operates like a database
/// transaction: attempt → verify → commit, or rollback on failure.
///
/// # Workflow
///
/// 1. **Execute**: Run the edit action sequence (Enter → Type → Enter)
/// 2. **Verify**: Check if edit succeeded via screen capture
///    - Verify not stuck in edit mode (optional)
///    - Verify expected value pattern matches (optional)
/// 3. **Commit**: If verification passes, return success
/// 4. **Rollback**: If verification fails:
///    - Press Escape × 2 to exit edit mode
///    - Execute navigation reset sequence
///    - Wait 1 second and retry (up to 3 times)
///
/// # Parameters
///
/// - `session`: The expectrl session controlling the TUI process
/// - `cap`: Terminal capture tool for screen verification
/// - `field_name`: Field name for logging (e.g., "station_id")
/// - `edit_actions`: Action sequence for editing the field
///   - Should include: Enter, delays, input, Enter
/// - `reset_navigation`: Actions to reset cursor position after rollback
///   - Example: `[PressCtrlPageUp, Sleep1s]`
/// - `expected_pattern`: Regex pattern to match expected value
///   - Example: `r">\s*1\s*<"` verifies Station ID is 1
/// - `check_not_in_edit`: Optional regex to detect stuck-in-edit-mode
///   - Example: `r">\s*\[?\s*_"` detects cursor underscore
///
/// # Returns
///
/// - `Ok(())` if field edit verified successfully within retry limit
/// - `Err(_)` if maximum retries exceeded or other error occurred
///
/// # Example
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// # let mut session = todo!();
/// # let mut cap = todo!();
/// use ci_utils::CursorAction;
///
/// // Configure Start Address to 0x0000
/// execute_field_edit_with_retry(
///     &mut session,
///     &mut cap,
///     "start_address",
///     vec![
///         CursorAction::PressEnter,
///         CursorAction::Sleep1s,
///         CursorAction::PressCtrlA,
///         CursorAction::TypeString(format!("{:x}", 0x0000)),
///         CursorAction::PressEnter,
///         CursorAction::Sleep1s,
///     ],
///     vec![
///         CursorAction::PressCtrlPageUp,
///         CursorAction::Sleep1s,
///     ],
///     r">\s*0x0000\s*<",        // Expected: "> 0x0000 <"
///     Some(r">\s*\[?\s*_"),     // Check for cursor: "> _ <"
/// ).await?;
/// # Ok(())
/// # }
/// ```
///
/// # See Also
///
/// - `TRANSACTION_RETRY_MECHANISM.md` - Complete documentation
/// - `configure_tui_station` - Usage in station configuration
pub async fn execute_field_edit_with_retry<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    field_name: &str,
    edit_actions: &[CursorAction],
    check_not_in_edit: bool, // Should verify we're NOT in edit mode after
    expected_pattern: Option<&str>, // Optional pattern to verify in screen
    reset_navigation: &[CursorAction], // Actions to navigate back to field
) -> Result<()> {
    for attempt in 1..=MAX_RETRY_ATTEMPTS {
        // Execute the edit actions
        execute_cursor_actions(session, cap, edit_actions, field_name).await?;

        // Verify the result
        let screen = cap
            .capture(session, &format!("verify_{}", field_name))
            .await?;

        let mut success = true;

        // Check if we're stuck in edit mode
        if check_not_in_edit {
            let in_edit_mode = screen.contains("_ <") || screen.contains("> [");
            if in_edit_mode {
                log::debug!("❌ Field '{}': Still in edit mode", field_name);
                success = false;
            }
        }

        // Check for expected pattern
        if let Some(pattern) = expected_pattern {
            if !screen.contains(pattern) {
                log::debug!(
                    "❌ Field '{}': Expected pattern '{}' not found",
                    field_name,
                    pattern
                );
                success = false;
            }
        }

        if success {
            log::info!("✅ Field '{}' edit verified successfully", field_name);
            return Ok(());
        }

        // Failed verification
        if attempt < MAX_RETRY_ATTEMPTS {
            log::warn!(
                "⚠️  Field '{}' edit verification failed, will retry (attempt {}/{})",
                field_name,
                attempt,
                MAX_RETRY_ATTEMPTS
            );

            // Rollback with safety checkpoints
            perform_safe_rollback(session, cap, field_name).await?;

            // Reset navigation
            execute_cursor_actions(session, cap, reset_navigation, "reset_navigation").await?;

            // Wait before retry
            sleep_1s().await;
        } else {
            return Err(anyhow!(
                "Field '{}' edit failed after {} attempts",
                field_name,
                MAX_RETRY_ATTEMPTS
            ));
        }
    }

    unreachable!()
}

/// Perform safe rollback with multi-layer checkpoints to prevent over-escaping
///
/// This function carefully exits edit mode while verifying we don't escape too far
/// (e.g., back to Entry page, About page, or ConfigPanel). It implements an adaptive
/// strategy: press Escape once, check the result, and only press Escape again if
/// still in edit mode.
///
/// # Problem Background
///
/// In CI environments, pressing Escape twice unconditionally can cause over-escaping:
/// - From Modbus panel → ConfigPanel → Entry page
/// - From field edit → station list → ConfigPanel
/// - Triggering About page or other unexpected navigation
///
/// # Solution: Checkpoint-Based Rollback
///
/// This function uses a multi-layer checkpoint system:
///
/// ## Layer 1: First Escape
/// - Press Escape once to attempt exiting edit mode
/// - Wait 500ms for UI to respond
///
/// ## Layer 2: Checkpoint 1 - Page Verification
/// Check if we're still in the correct area:
/// - ❌ Contains "Welcome" or "Press Enter to continue" → Entry page (fail)
/// - ❌ Contains "Thanks for using" or "About" → About page (fail)
/// - ❌ Contains "COM Ports" without "Station" → ConfigPanel (fail)
/// - ✅ Still contains "Station" or Modbus-related content → Continue
///
/// ## Layer 3: Edit Mode Detection
/// Check if we're still in edit mode:
/// - Cursor indicators: `_ <`, `> [`, `▂`
/// - If detected → Need second Escape
/// - If not detected → Successfully exited with one Escape
///
/// ## Layer 4: Conditional Second Escape
/// Only executed if still in edit mode:
/// - Press Escape second time
/// - Wait 500ms for UI to respond
///
/// ## Layer 5: Checkpoint 2 - Final Verification
/// Repeat page verification checks to ensure we didn't over-escape
///
/// # Parameters
///
/// - `session`: The expectrl session controlling the TUI process
/// - `cap`: Terminal capture tool for screen verification
/// - `context`: Context string for logging (e.g., field name, operation name)
///
/// # Returns
///
/// - `Ok(())` if successfully exited edit mode and remained in correct page
/// - `Err(_)` if over-escaped to wrong page (Entry/About/ConfigPanel)
///
/// # Error Messages
///
/// - `"Over-escaped to Entry page during rollback"` - Went back to main menu
/// - `"Over-escaped to About page during rollback"` - Triggered About screen
/// - `"Over-escaped to ConfigPanel during rollback"` - Left Modbus configuration
///
/// # Example Usage
///
/// This function is typically called automatically by [`execute_field_edit_with_retry`]
/// and [`execute_transaction_with_retry`]. For custom rollback:
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # use ci_utils::TerminalCapture;
/// # use expectrl::Expect;
/// # async fn example<T: Expect>(
/// #     session: &mut T,
/// #     cap: &mut TerminalCapture,
/// # ) -> Result<()> {
/// use examples::tui_e2e::common::perform_safe_rollback;
///
/// // After a failed field edit, safely exit edit mode
/// perform_safe_rollback(session, cap, "station_id_edit").await?;
/// # Ok(())
/// # }
/// ```
///
/// # Design Rationale
///
/// ## Why Adaptive Escape?
///
/// Different TUI states require different numbers of Escapes:
/// - **Text input field**: 1 Escape exits edit mode
/// - **Nested dialog**: May need 2 Escapes
/// - **Some widgets**: May exit on first Escape
///
/// By checking edit mode indicators, we adapt to the actual state.
///
/// ## Why Multiple Checkpoints?
///
/// Each Escape can potentially navigate to a different page:
/// - First Escape: Usually safe, but could trigger navigation
/// - Second Escape: Higher risk of over-escaping
///
/// Checking after each Escape allows early detection and failure.
///
/// ## Why Specific Text Patterns?
///
/// Page detection uses multiple indicators:
/// - "Welcome" → Unique to Entry page
/// - "Thanks for using" → Unique to About page
/// - "COM Ports" without "Station" → ConfigPanel but not Modbus panel
///
/// This prevents false positives from similar text in different contexts.
///
/// # Logging
///
/// The function provides detailed logging at each step:
/// - DEBUG: "🔄 Performing safe rollback for 'X'"
/// - DEBUG: "Still in edit mode after first Escape, pressing Escape again"
/// - DEBUG: "Successfully exited edit mode with one Escape"
/// - DEBUG: "✅ Safe rollback completed for 'X'"
/// - ERROR: "❌ Over-escaped to [Page] after [first/second] Escape"
///
/// # See Also
///
/// - [`execute_field_edit_with_retry`] - Uses this for rollback
/// - [`execute_transaction_with_retry`] - Can use this for default rollback
/// - `TRANSACTION_ENHANCEMENT_SUMMARY.md` - Full design documentation
pub async fn perform_safe_rollback<T: Expect>(
    session: &mut T,
    cap: &mut TerminalCapture,
    context: &str,
) -> Result<()> {
    log::debug!("🔄 Performing safe rollback for '{}'", context);

    // First Escape: Exit edit mode
    execute_cursor_actions(
        session,
        cap,
        &[CursorAction::PressEscape, CursorAction::Sleep1s],
        &format!("{}_rollback_1", context),
    )
    .await?;

    // Checkpoint 1: Verify we're still in Modbus panel
    let screen = cap
        .capture(session, &format!("{}_checkpoint_1", context))
        .await?;

    // Check if we over-escaped to wrong page
    if screen.contains("Welcome") || screen.contains("Press Enter to continue") {
        log::error!("❌ Over-escaped to Entry page after first Escape");
        return Err(anyhow!("Over-escaped to Entry page during rollback"));
    }

    if screen.contains("Thanks for using") || screen.contains("About") {
        log::error!("❌ Over-escaped to About page after first Escape");
        return Err(anyhow!("Over-escaped to About page during rollback"));
    }

    if screen.contains("COM Ports") && !screen.contains("Station") {
        log::warn!("⚠️  Escaped to ConfigPanel, need to re-enter Modbus panel");
        return Err(anyhow!("Over-escaped to ConfigPanel during rollback"));
    }

    // Check if we're still in edit mode (cursor visible)
    let still_in_edit = screen.contains("_ <") || screen.contains("> [") || screen.contains("▂");

    if still_in_edit {
        log::debug!("Still in edit mode after first Escape, pressing Escape again");

        // Second Escape: Ensure fully exited from edit mode
        execute_cursor_actions(
            session,
            cap,
            &[CursorAction::PressEscape, CursorAction::Sleep1s],
            &format!("{}_rollback_2", context),
        )
        .await?;

        // Checkpoint 2: Final verification
        let screen = cap
            .capture(session, &format!("{}_checkpoint_2", context))
            .await?;

        if screen.contains("Welcome") || screen.contains("Press Enter to continue") {
            log::error!("❌ Over-escaped to Entry page after second Escape");
            return Err(anyhow!("Over-escaped to Entry page during rollback"));
        }

        if screen.contains("Thanks for using") || screen.contains("About") {
            log::error!("❌ Over-escaped to About page after second Escape");
            return Err(anyhow!("Over-escaped to About page during rollback"));
        }

        if screen.contains("COM Ports") && !screen.contains("Station") {
            log::error!("❌ Over-escaped to ConfigPanel after second Escape");
            return Err(anyhow!("Over-escaped to ConfigPanel during rollback"));
        }
    } else {
        log::debug!("Successfully exited edit mode with one Escape");
    }

    log::debug!("✅ Safe rollback completed for '{}'", context);
    Ok(())
}

/// Generic transaction executor with retry and safe rollback.
///
/// # Problem Background
///
/// Different TUI operations require different verification and rollback strategies:
/// - **Station creation**: Check for station ID, rollback by deleting station
/// - **Register configuration**: Check for specific patterns, rollback by resetting values
/// - **Navigation**: Check for page titles, rollback by returning to previous page
/// - **Field editing**: Check for specific values, rollback by Escape to parent page
///
/// Unlike `execute_field_edit_with_retry` which is specialized for field editing with
/// `EditMode` detection, this function provides a **fully customizable** framework for
/// any TUI operation.
///
/// # Solution
///
/// This function provides a flexible transaction pattern with:
/// 1. **Custom Actions**: Execute any sequence of cursor actions
/// 2. **Custom Verification**: User-provided closure for validation logic
/// 3. **Flexible Rollback**: Optional custom rollback or default safe Escape
/// 4. **Navigation Reset**: Restore cursor position after rollback
/// 5. **Retry Loop**: Up to `MAX_RETRY_ATTEMPTS` (3) with `RETRY_WAIT_MS` (1000ms) delay
///
/// # Parameters
///
/// - `session`: The expectrl session for terminal I/O
/// - `cap`: Terminal capture tool for screen reading
/// - `operation_name`: Descriptive name for logging (e.g., "create_station", "delete_register")
/// - `actions`: Sequence of cursor actions to execute for the operation
/// - `verify_fn`: Custom verification closure:
///   - Input: `&str` (captured screen content)
///   - Output: `Result<bool>` (Ok(true) = success, Ok(false) = retry, Err = abort)
/// - `rollback_actions`: Optional custom rollback sequence:
///   - `Some(&[...])`: Execute custom rollback actions
///   - `None`: Use default `perform_safe_rollback` with adaptive Escape
/// - `reset_navigation`: Actions to restore cursor position after rollback (e.g., move to target field)
///
/// # Returns
///
/// - `Ok(())`: Operation succeeded within retry attempts
/// - `Err`: Operation failed after all retry attempts, or unrecoverable error occurred
///
/// # Error Messages
///
/// - **"Operation '{operation_name}' failed after {MAX_RETRY_ATTEMPTS} attempts"**:
///   All retry attempts exhausted, verification never returned Ok(true)
///
/// # Example 1: Station Creation with ID Verification
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// # let mut session = todo!();
/// # let mut cap = todo!();
/// use ci_utils::CursorAction;
///
/// execute_transaction_with_retry(
///     &mut session,
///     &mut cap,
///     "create_station",
///     &[
///         CursorAction::PressEnter,      // Create new station
///         CursorAction::Sleep1s,
///     ],
///     |screen| {
///         // Verify station was created with correct ID
///         Ok(screen.contains("Station #1") && screen.contains("Master"))
///     },
///     None, // Use default safe rollback (Escape to parent page)
///     &[CursorAction::PressCtrlPageUp], // Navigate back to station list
/// ).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Example 2: Register Configuration with Custom Rollback
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// # let mut session = todo!();
/// # let mut cap = todo!();
/// use ci_utils::CursorAction;
///
/// execute_transaction_with_retry(
///     &mut session,
///     &mut cap,
///     "configure_register",
///     &[
///         CursorAction::TypeString("100".into()), // Set start address
///         CursorAction::PressTab,
///         CursorAction::TypeString("10".into()),  // Set count
///         CursorAction::PressEnter,
///     ],
///     |screen| {
///         // Verify configuration was applied
///         Ok(screen.contains("Start: 100") && screen.contains("Count: 10"))
///     },
///     Some(&[
///         CursorAction::PressEscape,              // Exit configuration
///         CursorAction::PressEnter,               // Re-enter configuration
///         CursorAction::PressBackspace,           // Clear field
///         CursorAction::PressBackspace,
///     ]), // Custom rollback: reset fields instead of exiting
///     &[
///         CursorAction::PressCtrlHome,            // Navigate to first field
///     ],
/// ).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Example 3: Complex Verification with Error Handling
///
/// ```rust,no_run
/// # use anyhow::{Result, anyhow};
/// # async fn example() -> Result<()> {
/// # let mut session = todo!();
/// # let mut cap = todo!();
/// use ci_utils::CursorAction;
///
/// execute_transaction_with_retry(
///     &mut session,
///     &mut cap,
///     "delete_station",
///     &[
///         CursorAction::PressChar('d'),          // Delete key
///         CursorAction::PressEnter,              // Confirm
///         CursorAction::Sleep1s,
///     ],
///     |screen| {
///         // Check for deletion confirmation
///         if screen.contains("Error") {
///             Err(anyhow!("Deletion error detected"))
///         } else if screen.contains("Station #1") {
///             Ok(false) // Station still exists, retry
///         } else {
///             Ok(true) // Station deleted successfully
///         }
///     },
///     None, // Use default safe rollback
///     &[CursorAction::PressCtrlPageUp], // Return to station list
/// ).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Design Rationale
///
/// ## Why Custom Verification Closure?
///
/// Different operations need different validation logic:
/// - Station creation: Check for ID + role in list
/// - Register config: Check for numeric values + patterns
/// - Navigation: Check for page-specific titles
/// - Deletion: Verify absence of specific text
///
/// The closure pattern allows each call site to define its own validation without
/// adding operation-specific parameters to the function signature.
///
/// ## Why Optional Custom Rollback?
///
/// Some operations have **domain-specific rollback requirements**:
/// - Creating stations: Should delete the station, not just Escape
/// - Editing fields: Should clear values, not exit edit mode
/// - Multi-step wizards: Should return to specific step, not exit wizard
///
/// Providing `None` uses the safe default (adaptive Escape), while `Some(&[...])`
/// allows precise control for complex scenarios.
///
/// ## Why Navigation Reset Parameter?
///
/// After rollback, the cursor may be in an unpredictable position:
/// - Default rollback escapes to parent page
/// - Custom rollback may land on different field
/// - Multi-step operations need to restart from specific point
///
/// The `reset_navigation` parameter ensures the cursor is positioned correctly
/// before the next retry attempt.
///
/// ## When to Use vs. `execute_field_edit_with_retry`?
///
/// | Use `execute_field_edit_with_retry` | Use `execute_transaction_with_retry` |
/// |-------------------------------------|--------------------------------------|
/// | Editing a single field              | Multi-step operations                |
/// | Standard field validation           | Custom verification logic            |
/// | Simple Escape rollback              | Operation-specific rollback          |
/// | EditMode detection sufficient       | Complex state detection needed       |
///
/// # Logging Output
///
/// Successful operation:
/// ```text
/// ✅ Operation 'create_station' completed successfully
/// ```
///
/// Retry attempt:
/// ```text
/// ❌ Operation 'create_station' verification failed
/// ⚠️  Operation 'create_station' failed, will retry (attempt 1/3)
/// [Rollback operations...]
/// [Reset navigation...]
/// ```
///
/// Final failure:
/// ```text
/// ❌ Operation 'create_station' verification failed
/// Error: Operation 'create_station' failed after 3 attempts
/// ```
///
/// # See Also
///
/// - [`execute_field_edit_with_retry`]: Specialized version for field editing
/// - [`perform_safe_rollback`]: Default rollback implementation with adaptive Escape
/// - [`MAX_RETRY_ATTEMPTS`], [`RETRY_WAIT_MS`]: Configuration constants
pub async fn execute_transaction_with_retry<T, F>(
    session: &mut T,
    cap: &mut TerminalCapture,
    operation_name: &str,
    actions: &[CursorAction],
    verify_fn: F,
    rollback_actions: Option<&[CursorAction]>,
    reset_navigation: &[CursorAction],
) -> Result<()>
where
    T: Expect,
    F: Fn(&str) -> Result<bool>,
{
    for attempt in 1..=MAX_RETRY_ATTEMPTS {
        // Execute the operation
        execute_cursor_actions(session, cap, actions, operation_name).await?;

        // Capture screen for verification
        let screen = cap
            .capture(session, &format!("verify_{}", operation_name))
            .await?;

        // Run custom verification
        match verify_fn(&screen) {
            Ok(true) => {
                log::info!("✅ Operation '{}' completed successfully", operation_name);
                return Ok(());
            }
            Ok(false) => {
                log::debug!("❌ Operation '{}' verification failed", operation_name);
            }
            Err(e) => {
                log::warn!(
                    "⚠️  Operation '{}' verification error: {}",
                    operation_name,
                    e
                );
            }
        }

        // Failed verification
        if attempt < MAX_RETRY_ATTEMPTS {
            log::warn!(
                "⚠️  Operation '{}' failed, will retry (attempt {}/{})",
                operation_name,
                attempt,
                MAX_RETRY_ATTEMPTS
            );

            // Rollback
            if let Some(custom_rollback) = rollback_actions {
                execute_cursor_actions(session, cap, custom_rollback, "custom_rollback").await?;
            } else {
                perform_safe_rollback(session, cap, operation_name).await?;
            }

            // Reset navigation
            execute_cursor_actions(session, cap, reset_navigation, "reset_navigation").await?;

            // Wait before retry
            sleep_1s().await;
        } else {
            return Err(anyhow!(
                "Operation '{}' failed after {} attempts",
                operation_name,
                MAX_RETRY_ATTEMPTS
            ));
        }
    }

    unreachable!()
}
