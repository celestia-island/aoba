/// CLI subprocess manager for TUI
///
/// This module manages CLI subprocesses that handle actual serial port communication.
/// The TUI acts as a control shell, spawning and managing CLI processes via IPC.
use anyhow::{anyhow, Result};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
};

use crate::protocol::ipc::{IpcClient, IpcConnection, IpcMessage};

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
    pub data_source: Option<String>,
}

/// CLI mode (master or slave)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliMode {
    /// Slave mode - listen (acts as server, responds to requests)
    SlaveListen,
    /// Slave mode - poll (acts as client, polls external master for data)
    SlavePoll,
    /// Master mode (provides data and responds to poll requests)
    MasterProvide,
}

/// A managed CLI subprocess
pub struct ManagedSubprocess {
    pub config: CliSubprocessConfig,
    pub child: Child,
    pub ipc_socket_name: String,
    pub ipc_connection: Option<IpcConnection>,
    ipc_accept_thread: Option<std::thread::JoinHandle<Result<IpcConnection>>>,
    stdout_thread: Option<std::thread::JoinHandle<()>>,
    stderr_thread: Option<std::thread::JoinHandle<()>>,
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
        let ipc_socket_name = crate::protocol::ipc::generate_socket_name();

        log::info!(
            "Spawning CLI subprocess for port {} in mode {:?}",
            config.port_name,
            config.mode
        );

        // Setup IPC listener before spawning the process
        let ipc_client = IpcClient::listen(ipc_socket_name.clone())?;

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

        // Add IPC channel UUID
        args.push("--ipc-channel".to_string());
        args.push(ipc_socket_name.clone());

        log::debug!("CLI subprocess command: {:?} {:?}", exe_path, args);

        // Spawn the subprocess
        let mut child = Command::new(exe_path)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        log::info!("CLI subprocess spawned with PID: {:?}", child.id());

        let stdout_thread = child.stdout.take().map(|stdout| {
            let port_label = config.port_name.clone();
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stdout);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) => {
                            log::debug!("CLI[{port_label}] stdout closed");
                            break;
                        }
                        Ok(_) => {
                            let trimmed = line.trim_end_matches(['\r', '\n']);
                            if !trimmed.is_empty() {
                                log::info!("CLI[{port_label}] stdout: {trimmed}");
                            }
                        }
                        Err(err) => {
                            log::warn!("CLI[{port_label}] stdout reader error: {err}");
                            break;
                        }
                    }
                }
            })
        });

        let stderr_thread = child.stderr.take().map(|stderr| {
            let port_label = config.port_name.clone();
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stderr);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) => {
                            log::debug!("CLI[{port_label}] stderr closed");
                            break;
                        }
                        Ok(_) => {
                            let trimmed = line.trim_end_matches(['\r', '\n']);
                            if !trimmed.is_empty() {
                                log::warn!("CLI[{port_label}] stderr: {trimmed}");
                            }
                        }
                        Err(err) => {
                            log::warn!("CLI[{port_label}] stderr reader error: {err}");
                            break;
                        }
                    }
                }
            })
        });

        // Spawn thread to accept IPC connection
        let accept_thread = std::thread::spawn(move || ipc_client.accept());

        Ok(Self {
            config,
            child,
            ipc_socket_name,
            ipc_connection: None,
            ipc_accept_thread: Some(accept_thread),
            stdout_thread,
            stderr_thread,
        })
    }

    /// Try to complete IPC connection if still pending
    fn try_complete_ipc_connection(&mut self) -> Result<()> {
        if let Some(thread) = self.ipc_accept_thread.take() {
            if thread.is_finished() {
                match thread.join() {
                    Ok(Ok(conn)) => {
                        log::info!("Accepted IPC connection for port {}", self.config.port_name);
                        self.ipc_connection = Some(conn);
                    }
                    Ok(Err(e)) => {
                        log::error!("IPC accept failed for {}: {}", self.config.port_name, e);
                        return Err(e);
                    }
                    Err(_) => {
                        return Err(anyhow!("IPC accept thread panicked"));
                    }
                }
            } else {
                // Thread still running, put it back
                self.ipc_accept_thread = Some(thread);
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
                if let Err(err) = self.try_complete_ipc_connection() {
                    log::debug!(
                        "Failed to complete IPC connection for {}: {err:?}",
                        self.config.port_name
                    );
                }
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
            if let Err(err) = conn.try_recv() {
                log::debug!(
                    "IPC drain after kill failed for {}: {err}",
                    self.config.port_name
                );
            }
        }
        self.join_log_threads();
        Ok(())
    }

    fn join_log_threads(&mut self) {
        if let Some(handle) = self.stdout_thread.take() {
            if let Err(err) = handle.join() {
                log::debug!(
                    "CLI[{}] stdout thread join error: {err:?}",
                    self.config.port_name
                );
            }
        }
        if let Some(handle) = self.stderr_thread.take() {
            if let Err(err) = handle.join() {
                log::debug!(
                    "CLI[{}] stderr thread join error: {err:?}",
                    self.config.port_name
                );
            }
        }
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
            log::info!("Stopping existing subprocess for port {}", port_name);
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

    /// Get snapshot for a running subprocess
    pub fn snapshot(&self, port_name: &str) -> Option<SubprocessSnapshot> {
        self.processes.get(port_name).map(|sp| sp.snapshot())
    }

    /// Get the list of active subprocess port names
    pub fn active_ports(&self) -> Vec<String> {
        self.processes.keys().cloned().collect()
    }

    /// Shutdown all subprocesses
    pub fn shutdown_all(&mut self) {
        for (port_name, mut subprocess) in self.processes.drain() {
            log::info!("Shutting down subprocess for port {}", port_name);
            if let Err(err) = subprocess.kill() {
                log::warn!("Failed to kill subprocess for {}: {}", port_name, err);
            }
        }
    }
}

impl Drop for SubprocessManager {
    fn drop(&mut self) {
        self.shutdown_all();
    }
}
