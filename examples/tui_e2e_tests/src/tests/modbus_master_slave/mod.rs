// E2E test modules for modbus master-slave communication
mod modbus_config;
mod port_navigation;
mod register_ops;

use anyhow::{anyhow, Result};

use aoba::ci::{should_run_vcom_tests, spawn_expect_process, TerminalCapture};

/// Full test: master-slave modbus communication with register value verification
pub async fn test_modbus_master_slave_communication() -> Result<()> {
    if !should_run_vcom_tests() {
        log::info!("Skipping modbus master-slave communication test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting modbus master-slave communication test");

    // Spawn first TUI process (will be master on vcom1)
    let mut master_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn master TUI process: {}", err))?;

    aoba::ci::sleep_a_while().await;
    let mut master_cap = TerminalCapture::new(24, 80);

    // Spawn second TUI process (will be slave on vcom2)
    let mut slave_session = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn slave TUI process: {}", err))?;

    aoba::ci::sleep_a_while().await;
    let mut slave_cap = TerminalCapture::new(24, 80);

    // Navigate to ports
    port_navigation::navigate_to_vcom1(&mut master_session, &mut master_cap, "master").await?;
    port_navigation::navigate_to_vcom2(&mut slave_session, &mut slave_cap, "slave").await?;

    // Navigate to Modbus panel and configure modes
    modbus_config::configure_master_mode(&mut master_session, &mut master_cap, "master").await?;
    modbus_config::configure_slave_mode(&mut slave_session, &mut slave_cap, "slave").await?;

    // Wait a bit for initial setup
    aoba::ci::sleep_a_while().await;

    // Verify master registers are set correctly (sanity check)
    log::info!("🔍 Step 1: Verify master registers are set");
    if let Err(e) =
        register_ops::verify_master_registers(&mut master_session, &mut master_cap, "master").await
    {
        log::error!("Master register verification failed: {e}");
        return Err(e);
    }

    // Verify slave registers match master values
    log::info!("🔍 Step 2: Verify slave registers match master");
    match register_ops::verify_slave_registers(&mut slave_session, &mut slave_cap, "slave").await {
        Ok(_) => {
            log::info!("✅ SUCCESS: Slave registers match master values!");
            log::info!("🎉 Master-slave communication is working correctly!");
        }
        Err(e) => {
            log::error!("❌ EXPECTED FAILURE: {e}");
            log::error!("📝 This is normal on first run - the test framework is working correctly");
            log::error!("📝 The Modbus master-slave communication needs to be implemented/fixed");
            log::error!("📝 Use the diagnostic output above to identify the communication issues");
            return Err(e);
        }
    }

    // Cleanup: quit both processes with Ctrl+C
    use aoba::ci::auto_cursor::{execute_cursor_actions, CursorAction};
    let quit_actions = vec![CursorAction::CtrlC];

    execute_cursor_actions(
        &mut master_session,
        &mut master_cap,
        &quit_actions,
        "master",
    )
    .await?;
    execute_cursor_actions(&mut slave_session, &mut slave_cap, &quit_actions, "slave").await?;

    aoba::ci::sleep_a_while().await;

    log::info!("🧪 Modbus master-slave communication test completed");
    Ok(())
}
