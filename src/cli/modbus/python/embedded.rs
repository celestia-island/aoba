use anyhow::{anyhow, Result};
use std::time::{Duration, Instant};

use flume::{Receiver, Sender};

use super::{types::PythonOutput, PythonRunner};

/// Commands sent to the Python daemon thread
#[derive(Debug)]
enum DaemonCommand {
    Execute,
    Stop,
}

/// Responses from the Python daemon thread
#[derive(Debug, Clone)]
enum DaemonResponse {
    Success(PythonOutput),
    Error(String),
    Stopped,
}

/// Embedded RustPython VM runner using isolated tokio daemon thread
/// This implementation uses flume channels to communicate with a separate thread
/// that runs the RustPython VM, avoiding Send trait issues with the VM itself.
pub struct PythonEmbeddedRunner {
    script_path: String,
    reboot_interval_ms: u64,
    last_execution: Option<Instant>,
    active: bool,
    command_tx: Sender<DaemonCommand>,
    response_rx: Receiver<DaemonResponse>,
    daemon_handle: Option<std::thread::JoinHandle<()>>,
}

impl PythonEmbeddedRunner {
    pub fn new(script_path: String, initial_reboot_interval_ms: Option<u64>) -> Result<Self> {
        // Verify script exists and read content
        let script_content = std::fs::read_to_string(&script_path)
            .map_err(|e| anyhow!("Failed to read Python script {}: {}", script_path, e))?;

        log::info!("Initializing RustPython embedded mode for: {}", script_path);

        // Create channels for communication
        let (command_tx, command_rx) = flume::unbounded::<DaemonCommand>();
        let (response_tx, response_rx) = flume::unbounded::<DaemonResponse>();

        // Spawn dedicated thread for Python VM
        let daemon_handle = std::thread::spawn(move || {
            Self::daemon_thread(script_content, command_rx, response_tx);
        });

        Ok(Self {
            script_path,
            reboot_interval_ms: initial_reboot_interval_ms.unwrap_or(1000),
            last_execution: None,
            active: true,
            command_tx,
            response_rx,
            daemon_handle: Some(daemon_handle),
        })
    }

    /// Daemon thread that runs the Python VM
    /// This thread is isolated and doesn't need to be Send
    /// NOTE: RustPython 0.4 has limited stdout capture support.
    /// For now, embedded mode requires scripts to work without print() output.
    /// Future versions may add better IO handling.
    fn daemon_thread(
        script_content: String,
        command_rx: Receiver<DaemonCommand>,
        response_tx: Sender<DaemonResponse>,
    ) {
        use rustpython_vm::{self as vm, Interpreter};

        // Initialize interpreter once for this thread
        let interp = Interpreter::with_init(Default::default(), |vm| {
            vm.add_native_modules(rustpython_stdlib::get_module_inits());
        });

        // Main loop: wait for commands
        while let Ok(cmd) = command_rx.recv() {
            match cmd {
                DaemonCommand::Execute => {
                    let result = interp.enter(|vm| -> vm::PyResult<()> {
                        // Compile and run the script
                        let code_obj = rustpython_compiler::compile(
                            &script_content,
                            rustpython_compiler::Mode::Exec,
                            "<embedded>".to_string(),
                            Default::default(),
                        )
                        .map_err(|e| vm.new_runtime_error(format!("Compile error: {:?}", e)))?;

                        let code = vm.ctx.new_code(code_obj);
                        let scope = vm.new_scope_with_builtins();
                        vm.run_code_obj(code, scope)?;

                        Ok(())
                    });

                    match result {
                        Ok(_) => {
                            // For now, embedded mode doesn't capture stdout well in RustPython 0.4
                            // Return a minimal success response
                            // Users should use external mode for full functionality
                            let output = PythonOutput::new(Vec::new()).with_reboot_interval(1000);

                            if response_tx.send(DaemonResponse::Success(output)).is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            let error_msg = format!("Python execution failed: {:?}", e);
                            if response_tx.send(DaemonResponse::Error(error_msg)).is_err() {
                                break;
                            }
                        }
                    }
                }
                DaemonCommand::Stop => {
                    let _ = response_tx.send(DaemonResponse::Stopped);
                    break;
                }
            }
        }

        log::info!("Python daemon thread shutting down");
    }
}

impl PythonRunner for PythonEmbeddedRunner {
    fn execute(&mut self) -> Result<PythonOutput> {
        if !self.active {
            return Err(anyhow!("Runner is not active"));
        }

        // Check reboot interval
        if let Some(last_exec) = self.last_execution {
            let elapsed = last_exec.elapsed();
            let required = Duration::from_millis(self.reboot_interval_ms);
            if elapsed < required {
                return Err(anyhow!(
                    "Reboot interval not elapsed ({}ms remaining)",
                    (required - elapsed).as_millis()
                ));
            }
        }

        log::info!("Executing embedded Python script: {}", self.script_path);

        // Send execute command
        self.command_tx
            .send(DaemonCommand::Execute)
            .map_err(|e| anyhow!("Failed to send execute command: {}", e))?;

        // Wait for response with timeout
        let timeout = Duration::from_secs(30);
        let response = self
            .response_rx
            .recv_timeout(timeout)
            .map_err(|e| anyhow!("Failed to receive response: {}", e))?;

        match response {
            DaemonResponse::Success(output) => {
                // Update reboot interval from output
                if let Some(interval) = output.reboot_interval_ms {
                    self.reboot_interval_ms = interval;
                }

                self.last_execution = Some(Instant::now());
                Ok(output)
            }
            DaemonResponse::Error(err) => Err(anyhow!("Python execution error: {}", err)),
            DaemonResponse::Stopped => Err(anyhow!("Python daemon has stopped")),
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn stop(&mut self) {
        self.active = false;
        let _ = self.command_tx.send(DaemonCommand::Stop);

        // Wait for daemon thread to finish
        if let Some(handle) = self.daemon_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for PythonEmbeddedRunner {
    fn drop(&mut self) {
        self.stop();
    }
}
