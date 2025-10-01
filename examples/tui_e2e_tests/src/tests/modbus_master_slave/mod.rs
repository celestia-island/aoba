// E2E test modules for modbus master-slave communication
mod modbus_config;
mod port_navigation;
mod register_ops;

use anyhow::{anyhow, Result};

use aoba::ci::{should_run_vcom_tests, spawn_expect_process, TerminalCapture};

/// Smoke test: verify that we can spawn two TUI processes and occupy both vcom ports
pub async fn test_modbus_smoke_dual_process() -> Result<()> {
    if !should_run_vcom_tests() {
        log::info!("Skipping modbus dual process smoke test on this platform");
        return Ok(());
    }

    log::info!("🧪 Starting modbus dual process smoke test");

    // Spawn first TUI process
    let mut session1 = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn first TUI process: {}", err))?;

    aoba::ci::sleep_a_while().await;
    let mut cap1 = TerminalCapture::new(24, 80);

    // Spawn second TUI process
    let mut session2 = spawn_expect_process(&["--tui"])
        .map_err(|err| anyhow!("Failed to spawn second TUI process: {}", err))?;

    aoba::ci::sleep_a_while().await;
    let mut cap2 = TerminalCapture::new(24, 80);

    // Navigate to vcom1 in first process using auto_cursor
    port_navigation::navigate_to_vcom1(&mut session1, &mut cap1, "session1").await?;

    // Navigate to vcom2 in second process using auto_cursor
    port_navigation::navigate_to_vcom2(&mut session2, &mut cap2, "session2").await?;

    // Quit both processes using auto_cursor
    use aoba::ci::auto_cursor::{execute_cursor_actions, CursorAction};
    let quit_actions = vec![CursorAction::TypeChar('q')];
    
    execute_cursor_actions(&mut session1, &mut cap1, &quit_actions, "session1").await?;
    execute_cursor_actions(&mut session2, &mut cap2, &quit_actions, "session2").await?;

    aoba::ci::sleep_a_while().await;

    log::info!("🧪 Modbus dual process smoke test completed successfully");
    Ok(())
}

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

    // ========== Navigate to ports ==========
    port_navigation::navigate_to_vcom1(&mut master_session, &mut master_cap, "master").await?;
    port_navigation::navigate_to_vcom2(&mut slave_session, &mut slave_cap, "slave").await?;

    // ========== Navigate to Modbus panel and configure modes ==========
    modbus_config::configure_master_mode(&mut master_session, &mut master_cap, "master").await?;
    modbus_config::configure_slave_mode(&mut slave_session, &mut slave_cap, "slave").await?;

    // ========== Set and verify register values ==========
    register_ops::set_magic_number(&mut master_session, &mut master_cap, "master", 0xCAFE).await?;
    register_ops::verify_magic_number(&mut slave_session, &mut slave_cap, "slave", 0xCAFE).await?;

    // ========== Cleanup ==========
    use aoba::ci::auto_cursor::{execute_cursor_actions, CursorAction};
    let quit_actions = vec![CursorAction::TypeChar('q')];
    
    execute_cursor_actions(&mut master_session, &mut master_cap, &quit_actions, "master").await?;
    execute_cursor_actions(&mut slave_session, &mut slave_cap, &quit_actions, "slave").await?;

    aoba::ci::sleep_a_while().await;

    log::info!("🧪 Modbus master-slave communication test completed");
    Ok(())
}
