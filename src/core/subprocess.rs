/// CLI subprocess manager for core business logic
///
/// This module manages CLI subprocesses that handle actual serial port communication.
/// It can be used by any UI frontend (TUI, GUI, WebUI).
use anyhow::{anyhow, Result};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
};

use flume::Receiver;

use crate::{
    cli::{config::StationConfig, status::CliMode},
    core::task_manager::spawn_task,
    protocol::{
        ipc::{
            generate_socket_name, get_command_channel_name, IpcClient, IpcCommandClient,
            IpcConnection, IpcMessage,
        },
        status::debug_dump::is_debug_dump_enabled,
    },
};

// Named constant for command channel connect retries to avoid magic numbers
const COMMAND_CHANNEL_CONNECT_RETRIES: usize = 3;

/// Configuration for a CLI subprocess
#[derive(Debug, Clone)]
pub struct CliSubprocessConfig {
    pub port_name: String,
    pub mode: CliMode,
    pub station_id: u8,
    pub register_address: u16,
    pub register_length: u16,
    pub register_mode: String,
    pub baud_rate: u32,
    pub request_interval_ms: u32,
    pub timeout_ms: u32,
    pub data_source: Option<String>,
}

/// A managed CLI subprocess
pub struct ManagedSubprocess {
    pub config: CliSubprocessConfig,
    pub child: Child,
    pub ipc_socket_name: String,
    pub ipc_connection: Option<IpcConnection>,
    pub command_client: Option<IpcCommandClient>,
    // Use channels for async communication instead of thread handles
    ipc_accept_result: Option<Receiver<IpcConnection>>,
    command_connect_result: Option<Receiver<IpcCommandClient>>,
    // Log tasks are managed by task_manager and don't need explicit handles
    /// Channel to receive stderr logs from the subprocess
    pub stderr_receiver: Option<flume::Receiver<String>>,
}

/// Lightweight snapshot of a managed subprocess for reporting to Status
#[derive(Debug, Clone)]
pub struct SubprocessSnapshot {
    pub mode: CliMode,
    pub ipc_socket_name: String,
    pub pid: Option<u32>,
}

impl ManagedSubprocess {
    /// Spawn a new CLI subprocess with the given configuration
    pub fn spawn(config: CliSubprocessConfig) -> Result<Self> {
        // Generate unique IPC socket name
        let ipc_socket_name = generate_socket_name();

        log::info!(
            "Spawning CLI subprocess for port {} in mode {:?}",
            config.port_name,
            config.mode
        );

        // Setup IPC listener before spawning the process
        let _ipc_client = IpcClient::listen(ipc_socket_name.clone())?;

        // Get the current executable path
        let exe_path = std::env::current_exe()?;

        // Build CLI arguments
        let mut args = Vec::new();

        // Add mode-specific arguments
        match config.mode {
            CliMode::SlaveListen => {
                args.push("--slave-listen-persist".to_string());
                args.push(config.port_name.clone());
            }
            CliMode::SlavePoll => {
                args.push("--slave-poll-persist".to_string());
                args.push(config.port_name.clone());
            }
            CliMode::MasterProvide => {
                args.push("--master-provide-persist".to_string());
                args.push(config.port_name.clone());

                // Add data source if provided
                if let Some(ref data_source) = config.data_source {
                    args.push("--data-source".to_string());
                    args.push(data_source.clone());
                } else {
                    return Err(anyhow!("Master mode requires data-source"));
                }
            }
        }

        // Add common arguments
        args.push("--station-id".to_string());
        args.push(config.station_id.to_string());
        args.push("--register-address".to_string());
        args.push(config.register_address.to_string());
        args.push("--register-length".to_string());
        args.push(config.register_length.to_string());
        args.push("--register-mode".to_string());
        args.push(config.register_mode.clone());
        args.push("--baud-rate".to_string());
        args.push(config.baud_rate.to_string());
        args.push("--request-interval-ms".to_string());
        args.push(config.request_interval_ms.to_string());
        args.push("--timeout-ms".to_string());
        args.push(config.timeout_ms.to_string());

        // Add IPC channel UUID
        args.push("--ipc-channel".to_string());
        args.push(ipc_socket_name.clone());

        // If TUI is in debug CI E2E test mode, propagate to CLI subprocess
        if is_debug_dump_enabled() {
            args.push("--debug-ci-e2e-test".to_string());
        }

        // Spawn the subprocess
        let mut child = Command::new(exe_path)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        log::info!("CLI subprocess spawned with PID: {:?}", child.id());

        // Spawn log readers using task_manager (no need to store handles)
        if let Some(stdout) = child.stdout.take() {
            let port_label = config.port_name.clone();
            spawn_task(async move {
                let mut reader = BufReader::new(stdout);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) => {
                            break;
                        }
                        Ok(_) => {
                            let trimmed = line.trim_end_matches(['\r', '\n']);
                            if !trimmed.is_empty() {
                                log::info!("CLI[{}] stdout: {}", port_label, trimmed);
                            }
                        }
                        Err(err) => {
                            log::warn!("CLI[{}] stdout reader error: {}", port_label, err);
                            break;
                        }
                    }
                }
                Ok(())
            });
        }

        // Create channel for stderr logs
        let (stderr_tx, stderr_rx) = flume::unbounded();

        if let Some(stderr) = child.stderr.take() {
            let port_label = config.port_name.clone();
            let stderr_tx_clone = stderr_tx.clone();
            spawn_task(async move {
                let mut reader = BufReader::new(stderr);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) => {
                            break;
                        }
                        Ok(_) => {
                            let trimmed = line.trim_end_matches(['\r', '\n']);
                            if !trimmed.is_empty() {
                                log::warn!("CLI[{}] stderr: {}", port_label, trimmed);
                                // Send to channel for TUI to capture
                                let _ = stderr_tx_clone.send(trimmed.to_string());
                            }
                        }
                        Err(err) => {
                            log::warn!("CLI[{}] stderr reader error: {}", port_label, err);
                            break;
                        }
                    }
                }
                Ok(())
            });
        }

        // Drop the sender so receiver can detect when stderr is closed
        drop(stderr_tx);

        // Use channels for IPC connection results
        let (ipc_tx, ipc_rx) = flume::bounded(1);
        let socket_name = ipc_socket_name.clone();
        spawn_task(async move {
            // Recreate the IPC client inside the blocking task
            let result = match IpcClient::listen(socket_name) {
                Ok(client) => client.accept(),
                Err(e) => Err(e),
            };
            // Flatten the nested Result<Result<T, E>, JoinError> to Result<T, E>
            let flattened_result = match result {
                Ok(inner_result) => inner_result,
                Err(join_error) => return Err(anyhow!("Task join error: {}", join_error)),
            };
            ipc_tx.send(flattened_result)?;
            Ok(())
        });

        // Use channels for command connection results
        let (cmd_tx, cmd_rx) = flume::bounded(1);
        let command_channel_name = get_command_channel_name(&ipc_socket_name);
        spawn_task(async move {
            // Wait a bit for CLI to set up its command listener
            crate::utils::sleep::sleep_1s().await;

            // Try to connect with retries
            let mut result = Err(anyhow!("No connection attempts made"));
            for attempt in 1..=COMMAND_CHANNEL_CONNECT_RETRIES {
                match IpcCommandClient::connect(command_channel_name.clone()) {
                    Ok(client) => {
                        log::info!("Connected to CLI command channel on attempt {attempt}");
                        result = Ok(client);
                        break;
                    }
                    Err(_err) if attempt < COMMAND_CHANNEL_CONNECT_RETRIES => {
                        crate::utils::sleep::sleep_1s().await;
                    }
                    Err(_err) => {
                        log::warn!("Failed to connect to CLI command channel after {attempt} attempts: {_err}");
                        result = Err(_err);
                    }
                }
            }
            cmd_tx.send(result?)?;
            Ok(())
        });

        Ok(Self {
            config,
            child,
            ipc_socket_name,
            ipc_connection: None,
            command_client: None,
            ipc_accept_result: Some(ipc_rx),
            command_connect_result: Some(cmd_rx),
            stderr_receiver: Some(stderr_rx),
        })
    }

    /// Try to complete IPC connection if still pending
    fn try_complete_ipc_connection(&mut self) -> Result<()> {
        if let Some(rx) = self.ipc_accept_result.take() {
            match rx.try_recv() {
                Ok(conn) => {
                    log::info!("Accepted IPC connection for port {}", self.config.port_name);
                    self.ipc_connection = Some(conn);
                }
                Err(flume::TryRecvError::Empty) => {
                    // Still waiting, put it back
                    self.ipc_accept_result = Some(rx);
                }
                Err(flume::TryRecvError::Disconnected) => {
                    log::error!(
                        "IPC accept channel disconnected for {}",
                        self.config.port_name
                    );
                }
            }
        }

        // Also try to complete command client connection
        if let Some(rx) = self.command_connect_result.take() {
            match rx.try_recv() {
                Ok(client) => {
                    log::info!(
                        "✅ Connected to command channel for port {}",
                        self.config.port_name
                    );
                    self.command_client = Some(client);
                }
                Err(flume::TryRecvError::Empty) => {
                    // Still waiting, put it back
                    self.command_connect_result = Some(rx);
                }
                Err(flume::TryRecvError::Disconnected) => {
                    log::warn!(
                        "Command channel connect channel disconnected for {}",
                        self.config.port_name
                    );
                }
            }
        }

        Ok(())
    }

    /// Check if subprocess is still running
    pub fn is_alive(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(Some(status)) => {
                log::info!(
                    "CLI subprocess {} exited with status {:?}",
                    self.config.port_name,
                    status
                );
                false
            }
            Ok(None) => {
                // Child still running; opportunistically finish IPC handshake if ready
                if let Err(_err) = self.try_complete_ipc_connection() {}
                true
            }
            Err(err) => {
                log::warn!(
                    "Failed to poll CLI subprocess {}: {err}",
                    self.config.port_name
                );
                false
            }
        }
    }

    /// Try to receive an IPC message from the subprocess
    pub fn try_recv_ipc(&mut self) -> Result<Option<IpcMessage>> {
        self.try_complete_ipc_connection()?;

        if let Some(conn) = self.ipc_connection.as_mut() {
            match conn.try_recv() {
                Ok(Some(msg)) => Ok(Some(msg)),
                Ok(None) => Ok(None),
                Err(err) => {
                    log::warn!("IPC receive error for {}: {err}", self.config.port_name);
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }

    /// Try to receive stderr logs from the subprocess
    pub fn try_recv_stderr_logs(&mut self) -> Vec<String> {
        let mut logs = Vec::new();
        if let Some(ref rx) = self.stderr_receiver {
            while let Ok(line) = rx.try_recv() {
                logs.push(line);
            }
        }
        logs
    }

    /// Send a full stations configuration update to the subprocess via command channel
    ///
    /// # Parameters
    /// - `stations`: Station configurations to send
    /// - `reason`: Optional reason for update ("user_edit", "initial_config", "sync", "read_response")
    pub fn send_stations_update(
        &mut self,
        stations: &[StationConfig],
        reason: Option<&str>,
    ) -> Result<()> {
        self.try_complete_ipc_connection()?;

        if let Some(ref mut client) = self.command_client {
            // Serialize stations using postcard
            let stations_data = postcard::to_allocvec(stations)
                .map_err(|e| anyhow!("Failed to serialize stations: {e}"))?;

            let msg = if let Some(reason) = reason {
                IpcMessage::stations_update_with_reason(stations_data, reason)
            } else {
                IpcMessage::stations_update(stations_data)
            };
            client.send(&msg)?;
            log::info!(
                "Sent stations update ({} stations) to CLI subprocess for port {}, reason={:?}",
                stations.len(),
                self.config.port_name,
                reason
            );
            Ok(())
        } else {
            Err(anyhow!("Command channel not yet connected"))
        }
    }

    // Note: legacy per-register/per-config command helpers were removed in favor
    // of `send_stations_update` which sends the full stations configuration.

    /// Kill the subprocess
    pub fn kill(&mut self) -> Result<()> {
        log::info!("Killing CLI subprocess for port {}", self.config.port_name);
        let already_exited = matches!(self.child.try_wait(), Ok(Some(_)));
        if !already_exited {
            use std::io::ErrorKind;
            if let Err(err) = self.child.kill() {
                if err.kind() != ErrorKind::InvalidInput {
                    return Err(anyhow!(
                        "Failed to kill CLI subprocess {}: {err}",
                        self.config.port_name
                    ));
                }
            }
            if let Err(err) = self.child.wait() {
                if err.kind() != ErrorKind::InvalidInput {
                    log::warn!(
                        "Waiting for CLI subprocess {} after kill failed: {err}",
                        self.config.port_name
                    );
                }
            }
        }
        if let Some(mut conn) = self.ipc_connection.take() {
            // Drain any remaining message to ensure socket closes cleanly
            if let Err(_err) = conn.try_recv() {}
        }
        Ok(())
    }

    /// Snapshot current subprocess state for status updates
    pub fn snapshot(&self) -> SubprocessSnapshot {
        SubprocessSnapshot {
            mode: self.config.mode.clone(),
            ipc_socket_name: self.ipc_socket_name.clone(),
            pid: Some(self.child.id()),
        }
    }
}

impl Drop for ManagedSubprocess {
    fn drop(&mut self) {
        let _ = self.kill();
    }
}

/// Manager for all CLI subprocesses
pub struct SubprocessManager {
    processes: HashMap<String, ManagedSubprocess>,
}

impl Default for SubprocessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SubprocessManager {
    /// Create a new subprocess manager
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
        }
    }

    /// Start a subprocess for the given port with the given configuration
    pub fn start_subprocess(&mut self, config: CliSubprocessConfig) -> Result<()> {
        let port_name = config.port_name.clone();

        // If a subprocess already exists for this port, stop it first
        if self.processes.contains_key(&port_name) {
            log::info!("Stopping existing subprocess for port {port_name}");
            self.stop_subprocess(&port_name)?;
        }

        // Spawn new subprocess
        let subprocess = ManagedSubprocess::spawn(config)?;
        self.processes.insert(port_name, subprocess);

        Ok(())
    }

    /// Stop a subprocess for the given port
    pub fn stop_subprocess(&mut self, port_name: &str) -> Result<()> {
        if let Some(mut subprocess) = self.processes.remove(port_name) {
            subprocess.kill()?;
        }
        Ok(())
    }

    /// Check all subprocesses and remove any that have died
    pub fn reap_dead_processes(&mut self) -> Vec<(String, Option<std::process::ExitStatus>)> {
        let mut dead = Vec::new();

        let dead_ports: Vec<String> = self
            .processes
            .iter_mut()
            .filter_map(|(port, subprocess)| {
                if !subprocess.is_alive() {
                    Some(port.clone())
                } else {
                    None
                }
            })
            .collect();

        for port in dead_ports {
            if let Some(mut subprocess) = self.processes.remove(&port) {
                let exit_status = subprocess.child.try_wait().ok().flatten();
                dead.push((port, exit_status));
            }
        }

        dead
    }

    /// Poll IPC messages from all subprocesses
    pub fn poll_ipc_messages(&mut self) -> Vec<(String, IpcMessage)> {
        let mut messages = Vec::new();

        for (port_name, subprocess) in self.processes.iter_mut() {
            if let Ok(Some(msg)) = subprocess.try_recv_ipc() {
                messages.push((port_name.clone(), msg));
            }
        }

        messages
    }

    /// Poll stderr logs from all subprocesses
    pub fn poll_stderr_logs(&mut self) -> Vec<(String, Vec<String>)> {
        let mut all_logs = Vec::new();

        for (port_name, subprocess) in self.processes.iter_mut() {
            let logs = subprocess.try_recv_stderr_logs();
            if !logs.is_empty() {
                all_logs.push((port_name.clone(), logs));
            }
        }

        all_logs
    }

    /// Get snapshot for a running subprocess
    pub fn snapshot(&self, port_name: &str) -> Option<SubprocessSnapshot> {
        self.processes.get(port_name).map(|sp| sp.snapshot())
    }

    /// Get the list of active subprocess port names
    pub fn active_ports(&self) -> Vec<String> {
        self.processes.keys().cloned().collect()
    }

    /// Send full stations update to CLI subprocess via IPC
    /// This sends the complete station configuration for the port
    ///
    /// The caller must provide a callback to retrieve station configuration
    ///
    /// # Parameters
    /// - `port_name`: Name of the port to update
    /// - `get_stations`: Callback to retrieve station configuration
    /// - `reason`: Optional reason for update ("user_edit", "initial_config", "sync", "read_response")
    pub fn send_stations_update_for_port<F>(
        &mut self,
        port_name: &str,
        get_stations: F,
        reason: Option<&str>,
    ) -> Result<()>
    where
        F: FnOnce(&str) -> Result<Vec<StationConfig>>,
    {
        // Get current stations configuration from the caller
        let stations = get_stations(port_name)?;

        if stations.is_empty() {
            return Err(anyhow!("Port {port_name} has no stations"));
        }

        // Send to subprocess
        if let Some(subprocess) = self.processes.get_mut(port_name) {
            subprocess.send_stations_update(&stations, reason)?;
            log::info!(
                "✅ Sent stations update ({} stations) for {port_name}, reason={:?}",
                stations.len(),
                reason
            );
            Ok(())
        } else {
            Err(anyhow!("No subprocess found for port {port_name}"))
        }
    }

    // Per-register forwarding removed. Use
    // `send_stations_update_for_port` to synchronize full station state with CLI subprocesses.

    /// Shutdown all subprocesses
    pub fn shutdown_all(&mut self) {
        for (port_name, mut subprocess) in self.processes.drain() {
            log::info!("Shutting down subprocess for port {port_name}");
            if let Err(err) = subprocess.kill() {
                log::warn!("Failed to kill subprocess for {port_name}: {err}");
            }
        }
    }
}

impl Drop for SubprocessManager {
    fn drop(&mut self) {
        self.shutdown_all();
    }
}
