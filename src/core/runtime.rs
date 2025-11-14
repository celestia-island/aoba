/// Core runtime logic for managing CLI subprocesses and handling IPC
///
/// This module contains the main core thread logic that can be shared across
/// different UI frontends (TUI, GUI, WebUI).
use anyhow::{anyhow, Result};
use std::time::Duration;

use super::{
    bus::{CoreToUi, UiToCore},
    subprocess::{CliSubprocessConfig, SubprocessManager},
};

/// Configuration for the core runtime
pub struct CoreRuntimeConfig {
    /// Interval between automatic port scans
    pub scan_interval: Duration,
    /// Whether polling is enabled on startup
    pub polling_enabled: bool,
}

impl Default for CoreRuntimeConfig {
    fn default() -> Self {
        Self {
            scan_interval: Duration::from_secs(30),
            polling_enabled: true,
        }
    }
}

/// Core runtime context that can be customized by different UI frontends
pub trait CoreContext: Send {
    /// Called when a port scan is requested
    fn scan_ports(&mut self) -> Result<()>;

    /// Called to get subprocess configuration for starting a runtime
    fn get_runtime_config(&self, port_name: &str) -> Result<Option<RuntimeStartConfig>>;

    /// Called when a subprocess is started successfully
    fn on_runtime_started(&mut self, port_name: &str, pid: Option<u32>) -> Result<()>;

    /// Called when a subprocess is stopped
    fn on_runtime_stopped(&mut self, port_name: &str) -> Result<()>;

    /// Called when a subprocess exits unexpectedly
    fn on_runtime_exited(
        &mut self,
        port_name: &str,
        exit_status: Option<std::process::ExitStatus>,
    ) -> Result<()>;

    /// Called when an IPC message is received from a subprocess
    fn handle_ipc_message(
        &mut self,
        port_name: &str,
        message: crate::protocol::ipc::IpcMessage,
    ) -> Result<()>;

    /// Called periodically to allow context to perform updates
    fn tick(&mut self) -> Result<()>;
}

/// Configuration needed to start a runtime for a port
pub struct RuntimeStartConfig {
    pub cli_config: CliSubprocessConfig,
    pub data_source_path: Option<std::path::PathBuf>,
}

/// Run the core thread with a given context
///
/// This is the main loop that handles:
/// - Processing UI messages
/// - Managing CLI subprocesses
/// - Polling IPC messages
/// - Periodic port scanning
pub fn run_core_thread<C: CoreContext>(
    ui_rx: flume::Receiver<UiToCore>,
    core_tx: flume::Sender<CoreToUi>,
    input_kill_tx: flume::Sender<()>,
    config: CoreRuntimeConfig,
    mut context: C,
) -> Result<()> {
    let mut polling_enabled = config.polling_enabled;
    let scan_interval = config.scan_interval;
    let mut last_scan = std::time::Instant::now() - scan_interval;

    let mut subprocess_manager = SubprocessManager::new();

    loop {
        // Process all pending messages from UI
        while let Ok(msg) = ui_rx.try_recv() {
            match msg {
                UiToCore::Quit => {
                    log::info!("Received quit signal");
                    subprocess_manager.shutdown_all();

                    // Notify context about shutdown
                    for port in subprocess_manager.active_ports() {
                        let _ = context.on_runtime_stopped(&port);
                    }

                    let _ = input_kill_tx.send(());
                    core_tx
                        .send(CoreToUi::Quit)
                        .map_err(|err| anyhow!("Failed to send Quit to UI: {err}"))?;
                    return Ok(());
                }
                UiToCore::Refresh => {
                    core_tx
                        .send(CoreToUi::Refreshed)
                        .map_err(|err| anyhow!("Failed to send Refreshed: {err}"))?;
                }
                UiToCore::RescanPorts => {
                    if let Err(err) = context.scan_ports() {
                        log::warn!("Port scan failed: {err}");
                    }
                    last_scan = std::time::Instant::now();
                    core_tx
                        .send(CoreToUi::Refreshed)
                        .map_err(|err| anyhow!("Failed to send Refreshed: {err}"))?;
                }
                UiToCore::PausePolling => {
                    polling_enabled = false;
                    core_tx
                        .send(CoreToUi::Refreshed)
                        .map_err(|err| anyhow!("Failed to send Refreshed: {err}"))?;
                }
                UiToCore::ResumePolling => {
                    polling_enabled = true;
                    core_tx
                        .send(CoreToUi::Refreshed)
                        .map_err(|err| anyhow!("Failed to send Refreshed: {err}"))?;
                }
                UiToCore::ToggleRuntime(port_name) => {
                    log::info!("ToggleRuntime requested for {port_name}");

                    // Check if already running
                    if subprocess_manager.snapshot(&port_name).is_some() {
                        // Stop it
                        if let Err(err) = subprocess_manager.stop_subprocess(&port_name) {
                            log::warn!("Failed to stop subprocess for {port_name}: {err}");
                        }
                        let _ = context.on_runtime_stopped(&port_name);
                    } else {
                        // Start it
                        if let Ok(Some(config)) = context.get_runtime_config(&port_name) {
                            match subprocess_manager.start_subprocess(config.cli_config) {
                                Ok(()) => {
                                    if let Some(snapshot) = subprocess_manager.snapshot(&port_name)
                                    {
                                        let _ =
                                            context.on_runtime_started(&port_name, snapshot.pid);

                                        // Send initial stations update
                                        // Note: This would need to be implemented by the context
                                        // For now, we skip this as the context should handle it
                                        log::debug!("Subprocess started for {port_name}, context should send initial config");
                                    }
                                }
                                Err(err) => {
                                    log::error!(
                                        "Failed to start subprocess for {port_name}: {err}"
                                    );
                                }
                            }
                        }
                    }
                    core_tx
                        .send(CoreToUi::Refreshed)
                        .map_err(|err| anyhow!("Failed to send Refreshed: {err}"))?;
                }
                UiToCore::RestartRuntime(port_name) => {
                    log::info!("RestartRuntime requested for {port_name}");

                    // Stop if running
                    if subprocess_manager.snapshot(&port_name).is_some() {
                        if let Err(err) = subprocess_manager.stop_subprocess(&port_name) {
                            log::warn!("Failed to stop subprocess for {port_name}: {err}");
                        }
                    }

                    // Start it
                    if let Ok(Some(config)) = context.get_runtime_config(&port_name) {
                        match subprocess_manager.start_subprocess(config.cli_config) {
                            Ok(()) => {
                                if let Some(snapshot) = subprocess_manager.snapshot(&port_name) {
                                    let _ = context.on_runtime_started(&port_name, snapshot.pid);

                                    // Send initial stations update
                                    // Note: This would need to be implemented by the context
                                    log::debug!("Subprocess restarted for {port_name}, context should send initial config");
                                }
                            }
                            Err(err) => {
                                log::error!("Failed to start subprocess for {port_name}: {err}");
                            }
                        }
                    }
                    core_tx
                        .send(CoreToUi::Refreshed)
                        .map_err(|err| anyhow!("Failed to send Refreshed: {err}"))?;
                }
                UiToCore::SendRegisterUpdate { port_name, .. } => {
                    log::info!("SendRegisterUpdate requested for {port_name}");
                    // Context should handle sending the update via the subprocess manager
                    // We just notify the context through the normal flow
                }
            }
        }

        // Reap dead processes
        let dead_processes = subprocess_manager.reap_dead_processes();
        for (port_name, exit_status) in dead_processes {
            let _ = context.on_runtime_exited(&port_name, exit_status);
            core_tx
                .send(CoreToUi::Refreshed)
                .map_err(|err| anyhow!("Failed to send Refreshed: {err}"))?;
        }

        // Poll IPC messages
        for (port_name, message) in subprocess_manager.poll_ipc_messages() {
            if let Err(err) = context.handle_ipc_message(&port_name, message) {
                log::warn!("Failed to handle IPC message for {port_name}: {err}");
            }
        }

        // Periodic port scanning
        if polling_enabled && last_scan.elapsed() >= scan_interval {
            if let Err(err) = context.scan_ports() {
                log::warn!("Port scan failed: {err}");
            }
            last_scan = std::time::Instant::now();
        }

        // Allow context to perform periodic updates
        if let Err(err) = context.tick() {
            log::warn!("Context tick failed: {err}");
        }

        // Send tick to UI
        core_tx
            .send(CoreToUi::Tick)
            .map_err(|err| anyhow!("Failed to send Tick: {err}"))?;

        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
