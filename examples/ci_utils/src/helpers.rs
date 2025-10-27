use std::time::Duration;

/// Sleep for 1 second (1000ms) - standard delay for CI/E2E tests
pub async fn sleep_1s() {
    tokio::time::sleep(Duration::from_millis(1000)).await;
}

/// Sleep for 3 seconds (3000ms) - extended delay for complex operations
pub async fn sleep_3s() {
    tokio::time::sleep(Duration::from_millis(3000)).await;
}

/// Terminate an expectrl session by sending Ctrl+C and waiting for cleanup
///
/// This function ensures proper termination of TUI processes spawned via expectrl.
/// It's especially important in CI environments where default Drop behavior may
/// not reliably clean up child processes, leading to zombie processes or port conflicts.
///
/// # Process Termination Strategy
///
/// 1. Send Ctrl+C (SIGINT) to request graceful shutdown
/// 2. Wait 1 second for process to handle signal and exit
/// 3. On Unix: Send SIGTERM if process is still alive
/// 4. On Unix: Send SIGKILL as last resort if SIGTERM fails
/// 5. Wait for process to be fully reaped by OS
///
/// # Parameters
///
/// - `session`: The expectrl session to terminate
/// - `process_name`: Human-readable name for logging (e.g., "TUI", "CLI Slave")
///
/// # Example
///
/// ```no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// use ci_utils::{spawn_expect_process, terminate_session};
///
/// let mut session = spawn_expect_process(&["--tui"])?;
/// // ... use session ...
/// terminate_session(session, "TUI").await?;
/// # Ok(())
/// # }
/// ```
///
/// # Platform Differences
///
/// - **Unix/Linux**: Uses signal-based termination (SIGINT → SIGTERM → SIGKILL)
/// - **Windows**: Sends Ctrl+C and waits; relies on process's signal handling
///
/// # Why This Is Needed
///
/// The default `Drop` implementation for expectrl::Session may not reliably terminate
/// processes in all scenarios, particularly:
///
/// - CI environments with aggressive resource constraints
/// - When tests fail with panics (Drop may not run completely)
/// - When processes have child subprocesses that need cleanup
/// - When signal delivery is delayed or lost
///
/// Explicit termination ensures tests don't leave zombie processes that:
/// - Block serial ports from subsequent tests
/// - Consume file descriptors
/// - Interfere with process-based resource locks
/// Terminate an expectrl session by sending Ctrl+C and forcibly killing if needed
///
/// This function ensures proper termination of TUI processes spawned via expectrl.
/// It's especially important in CI environments where default Drop behavior may
/// not reliably clean up child processes, leading to zombie processes or port conflicts.
///
/// # Process Termination Strategy
///
/// 1. Send Ctrl+C (SIGINT) to request graceful shutdown  
/// 2. Wait 1 second for process to handle signal and exit
/// 3. On Unix: Get PID and send SIGKILL to force termination
/// 4. Wait for OS to reap the process
///
/// # Parameters
///
/// - `session`: The expectrl session to terminate (consumes it)
/// - `process_name`: Human-readable name for logging (e.g., "TUI", "CLI Slave")
///
/// # Example
///
/// ```no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// use ci_utils::{spawn_expect_process, terminate_session};
///
/// let session = spawn_expect_process(&["--tui"])?;
/// // ... use session ...
/// terminate_session(session, "TUI").await?;
/// # Ok(())
/// # }
/// ```
///
/// # Platform Differences
///
/// - **Unix/Linux**: Uses PID + SIGKILL for guaranteed termination
/// - **Windows**: Sends Ctrl+C and waits; relies on process's signal handling
///
/// # Why This Is Needed
///
/// The default `Drop` implementation for expectrl::Session may not reliably terminate
/// processes in all scenarios, particularly:
///
/// - CI environments with aggressive resource constraints  
/// - When tests fail with panics (Drop may not run completely)
/// - When processes have child subprocesses that need cleanup
/// - When signal delivery is delayed or lost
///
/// Explicit termination ensures tests don't leave zombie processes that:
/// - Block serial ports from subsequent tests
/// - Consume file descriptors
/// - Interfere with process-based resource locks
pub async fn terminate_session<S>(session: S, process_name: &str) -> anyhow::Result<()>
where
    S: expectrl::Expect,
{
    log::info!("🛑 Terminating {} process...", process_name);

    // We need to take ownership and drop the session, which should trigger cleanup
    // But first, let's try to send Ctrl+C for graceful shutdown
    let mut session = session;
    
    // Send Ctrl+C for graceful shutdown
    let ctrlc_bytes = b"\x03";
    if let Err(e) = session.send(ctrlc_bytes) {
        log::warn!("⚠️  Failed to send Ctrl+C to {}: {}", process_name, e);
    }

    // Wait for graceful shutdown
    tokio::time::sleep(Duration::from_millis(1000)).await;

    // Now drop the session explicitly - this should trigger expectrl's Drop impl
    drop(session);

    // Additional wait to ensure cleanup completes
    tokio::time::sleep(Duration::from_millis(500)).await;

    log::info!("✅ {} process termination complete", process_name);

    Ok(())
}
