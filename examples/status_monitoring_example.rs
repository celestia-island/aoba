/// Example demonstrating status monitoring for TUI E2E testing
///
/// This example shows how to use the new status monitoring utilities to verify
/// TUI/CLI behavior without relying on terminal screen capture.
///
/// Run with: cargo run --example status_monitoring_example
use anyhow::Result;

use ci_utils::{read_tui_status, spawn_expect_process, wait_for_tui_page};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::init();

    println!("📚 Status Monitoring Example");
    println!("This example demonstrates how to use status monitoring in TUI E2E tests.");
    println!();

    // Step 1: Enable debug mode by setting environment variable
    std::env::set_var("AOBA_DEBUG_CI_E2E_TEST", "1");
    println!("✅ Enabled debug mode (AOBA_DEBUG_CI_E2E_TEST=1)");

    // Step 2: Spawn TUI process
    println!("🚀 Spawning TUI process...");
    let mut _tui_session = spawn_expect_process(&["--tui"])?;

    // Wait a bit for TUI to initialize and start writing status
    println!("⏳ Waiting for TUI to initialize...");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Step 3: Use status monitoring to verify TUI state

    // Wait for TUI to reach Entry page (with 10 second timeout)
    println!("🔍 Waiting for TUI to reach Entry page...");
    let status = wait_for_tui_page("Entry", 10, None).await?;
    println!("✅ TUI is on Entry page");
    println!(
        "   Ports available: {:?}",
        status.ports.iter().map(|p| &p.name).collect::<Vec<_>>()
    );

    // Read current TUI status directly
    println!("\n📊 Reading current TUI status:");
    let current_status = read_tui_status()?;
    println!("   Current page: {}", current_status.page);
    println!("   Number of ports: {}", current_status.ports.len());

    // Show details about each port
    for port in &current_status.ports {
        println!("\n   Port: {}", port.name);
        println!("     Enabled: {}", port.enabled);
        println!("     State: {}", port.state);
        println!("     Masters: {}", port.modbus_masters.len());
        println!("     Slaves: {}", port.modbus_slaves.len());
        println!("     Logs: {}", port.log_count);
    }

    println!("\n✅ Example completed successfully!");
    println!("   Status dump file: /tmp/tui_e2e.log");
    println!("   You can inspect the JSON file to see the status structure.");

    Ok(())
}
