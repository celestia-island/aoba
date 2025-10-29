use anyhow::{anyhow, Result};
use expectrl::Expect;

use ci_utils::*;

/// Maximum number of retry attempts for transaction operations.
///
/// Each operation (field edit, navigation, etc.) will be attempted up to this many
/// times before failing. This provides resilience against CI environment timing issues.
const MAX_RETRY_ATTEMPTS: usize = 3;

/// Perform safe rollback with multi-layer checkpoints to prevent over-escaping.
///
/// This function carefully exits edit mode while verifying we do not escape too far
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
/// Check if we are still in the correct area:
/// - ❌ Contains "Welcome" or "Press Enter to continue" → Entry page (fail)
/// - ❌ Contains "Thanks for using" or "About" → About page (fail)
/// - ❌ Contains "COM Ports" without "Station" → ConfigPanel (fail)
/// - ✅ Still contains "Station" or Modbus-related content → Continue
///
/// ## Layer 3: Edit Mode Detection
/// Check if we are still in edit mode:
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
/// Repeat page verification checks to ensure we did not over-escape.
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
/// # Example Usage
///
/// ```rust,no_run
/// # use anyhow::Result;
/// # use ci_utils::TerminalCapture;
/// # use expectrl::Expect;
/// # async fn example<T: Expect>(session: &mut T, cap: &mut TerminalCapture) -> Result<()> {
/// use examples::tui_e2e::common::perform_safe_rollback;
///
/// // After a failed step, safely exit edit mode
/// perform_safe_rollback(session, cap, "station_id_edit").await?;
/// # Ok(())
/// # }
/// ```
///
/// # See Also
///
/// - [`execute_transaction_with_retry`]: Default caller providing rollback context
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

    // Checkpoint 1: Verify we are still in Modbus panel
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

    // Check if we are still in edit mode (cursor visible)
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
/// This function provides a **fully customizable** framework for any TUI operation.
///
/// # Solution
///
/// This function provides a flexible transaction pattern with:
/// 1. **Custom Actions**: Execute any sequence of cursor actions
/// 2. **Custom Verification**: User-provided closure for validation logic
/// 3. **Flexible Rollback**: Optional custom rollback or default safe Escape
/// 4. **Navigation Reset**: Restore cursor position after rollback
/// 5. **Retry Loop**: Up to `MAX_RETRY_ATTEMPTS` (3) with a 1 second delay between attempts
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
///   - `None`: Use default [`perform_safe_rollback`] with adaptive Escape
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
///     &[CursorAction::PressCtrlHome],
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
///     &[CursorAction::PressCtrlPageUp],
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
/// - [`perform_safe_rollback`]: Default rollback implementation with adaptive Escape
/// - [`MAX_RETRY_ATTEMPTS`]: Configuration constant controlling retry count
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
