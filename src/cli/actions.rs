use anyhow::anyhow;
use serde::Serialize;

use clap::ArgMatches;

use crate::{
    core::task_manager::spawn_task,
    protocol::ipc::{self, IpcCommandListener, IpcServer},
    utils::sleep::sleep_1s,
};

/// IPC connections for CLI subprocess (bidirectional)
pub struct IpcConnections {
    pub status: IpcServer,
    pub command_listener: IpcCommandListener,
}

/// Check if a port is occupied using Windows API (Windows) or file lock checking (Unix)
/// Returns true if occupied, false if free
fn check_port_occupation(port_name: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows::{
            core::PCWSTR,
            Win32::{
                Foundation::{CloseHandle, GetLastError, WIN32_ERROR},
                Storage::FileSystem::{
                    CreateFileW, FILE_FLAG_OVERLAPPED, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
                    FILE_SHARE_NONE, OPEN_EXISTING,
                },
            },
        };

        // Convert COM port name to Windows device path (e.g., "COM3" -> "\\.\\COM3")
        let device_path = if port_name.starts_with("\\\\.\\\\") {
            port_name.to_string()
        } else {
            format!("\\\\.\\{port_name}")
        };

        // Convert to UTF-16 for Windows API
        let wide_path: Vec<u16> = device_path
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        // Try to open port with exclusive access (FILE_SHARE_NONE)
        // If another process has it open, this will fail with ERROR_SHARING_VIOLATION
        unsafe {
            let handle = CreateFileW(
                PCWSTR(wide_path.as_ptr()),
                FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
                FILE_SHARE_NONE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                None,
            );

            if handle.is_err() {
                // Failed to open, check error code
                let error = GetLastError();

                // ERROR_SHARING_VIOLATION (32) means port is occupied by another process
                // ERROR_ACCESS_DENIED (5) may also indicate occupation
                let is_occupied = matches!(error, WIN32_ERROR(32) | WIN32_ERROR(5));

                log::debug!(
                    "Windows API CreateFileW failed for {port_name}: error={error:?}, occupied={is_occupied}",
                );

                return is_occupied;
            }

            // Successfully opened, close it and return free
            let handle = handle.unwrap();
            let _ = CloseHandle(handle);
            log::debug!("Windows API successfully opened {port_name}, port is free");
            false
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        use std::fs;
        use std::os::unix::fs::MetadataExt;
        use std::path::{Path, PathBuf};

        fn canonical_device_path(port_path: &str) -> Option<PathBuf> {
            match fs::canonicalize(port_path) {
                Ok(path) => Some(path),
                Err(err) => {
                    log::warn!(
                        "Unable to canonicalize {}: {} (will attempt raw path)",
                        port_path,
                        err
                    );
                    let candidate = Path::new(port_path);
                    if candidate.is_absolute() {
                        Some(candidate.to_path_buf())
                    } else {
                        std::env::current_dir().ok().map(|cwd| cwd.join(candidate))
                    }
                }
            }
        }

        fn device_rdev(path: &Path) -> Option<u64> {
            fs::metadata(path).map(|meta| meta.rdev()).ok()
        }

        let target_path = match canonical_device_path(port_name) {
            Some(p) => p,
            None => {
                log::warn!(
                    "Cannot resolve device path for {} — assuming free",
                    port_name
                );
                return false;
            }
        };

        let target_rdev = match device_rdev(&target_path) {
            Some(dev) if dev != 0 => dev,
            _ => {
                log::warn!(
                    "Unable to obtain device id for {} — assuming free",
                    target_path.display()
                );
                return false;
            }
        };

        let self_pid = std::process::id();
        if let Ok(proc_entries) = fs::read_dir("/proc") {
            for entry in proc_entries.flatten() {
                let file_name = entry.file_name();
                let pid_str = file_name.to_string_lossy();
                let pid: u32 = match pid_str.parse() {
                    Ok(pid) => pid,
                    Err(_) => continue,
                };

                if pid == self_pid {
                    continue;
                }

                let fd_dir = entry.path().join("fd");
                let fd_iter = match fs::read_dir(&fd_dir) {
                    Ok(iter) => iter,
                    Err(_) => continue,
                };

                for fd_entry in fd_iter.flatten() {
                    let fd_path = fd_entry.path();

                    // First try comparing by device id when accessible
                    if let Ok(meta) = fs::metadata(&fd_path) {
                        let rdev = meta.rdev();
                        if rdev == target_rdev {
                            log::debug!(
                                "Detected matching device id for {} via {}",
                                port_name,
                                fd_path.display()
                            );
                            return true;
                        }
                    }

                    // Fallback to comparing canonicalized link targets
                    if let Ok(link) = fs::read_link(&fd_path) {
                        if let Ok(canon) = fs::canonicalize(&link) {
                            if canon == target_path {
                                log::debug!(
                                    "Detected open handle to {} by PID {} (fd: {})",
                                    port_name,
                                    pid,
                                    fd_path.display()
                                );
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }
}

/// Helper to establish IPC connections if requested (bidirectional)
pub fn setup_ipc(matches: &ArgMatches) -> Option<IpcConnections> {
    if let Some(channel_id) = matches.get_one::<String>("ipc-channel") {
        log::info!("IPC: Attempting to connect to status channel: {channel_id}");
        match IpcServer::connect(channel_id.clone()) {
            Ok(status) => {
                log::info!("IPC: Successfully connected to status channel");

                // Create command listener on the reverse channel
                let command_channel = ipc::get_command_channel_name(channel_id);
                log::info!("IPC: Creating command listener on: {command_channel}");
                match IpcCommandListener::listen(command_channel) {
                    Ok(command_listener) => {
                        log::info!("IPC: Command listener created successfully");
                        Some(IpcConnections {
                            status,
                            command_listener,
                        })
                    }
                    Err(err) => {
                        log::warn!("IPC: Failed to create command listener: {err}");
                        None
                    }
                }
            }
            Err(err) => {
                log::warn!("IPC: Failed to connect to status channel: {err}");
                None
            }
        }
    } else {
        None
    }
}

#[derive(Serialize)]
struct PortInfo<'a> {
    #[serde(rename = "path")]
    port_name: &'a str,
    status: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    guid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vid: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pid: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    serial: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    annotation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    canonical: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    raw_port_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    product: Option<String>,
}

pub async fn run_one_shot_actions(matches: &ArgMatches) -> bool {
    // Handle check-port command (must be before list-ports)
    if let Some(port_name) = matches.get_one::<String>("check-port") {
        let is_occupied = check_port_occupation(port_name);

        let want_json = matches.get_flag("json");
        if want_json {
            #[derive(serde::Serialize)]
            struct CheckResult {
                port: String,
                occupied: bool,
                status: &'static str,
            }
            let result = CheckResult {
                port: port_name.clone(),
                occupied: is_occupied,
                status: if is_occupied { "Occupied" } else { "Free" },
            };
            if let Ok(s) = serde_json::to_string(&result) {
                println!("{s}");
            }
        } else {
            // Plain text output
            if is_occupied {
                eprintln!("Port {port_name} is occupied");
            } else {
                println!("Port {port_name} is free");
            }
        }

        // Exit with appropriate code: 0 = free, 1 = occupied
        std::process::exit(if is_occupied { 1 } else { 0 });
    }

    if matches.get_flag("list-ports") {
        let ports_enriched = crate::protocol::tty::available_ports_enriched();

        let want_json = matches.get_flag("json");
        if want_json {
            let mut out: Vec<PortInfo> = Vec::new();

            // Note: CLI one-shot commands don't have access to status tree,
            // so we always report ports as "Free" in this context
            for (p, extra) in ports_enriched.iter() {
                let status = "Free";

                // Attempt to capture annotation if present in port_name (parenthetical)
                let ann = if p.port_name.contains('(') && p.port_name.contains(')') {
                    Some(p.port_name.clone())
                } else {
                    None
                };
                // Canonical: COMn if present, else basename for unix-like
                let canonical = compute_canonical(&p.port_name);
                let raw_type = Some(format!("{:?}", p.port_type));

                out.push(PortInfo {
                    port_name: &p.port_name,
                    status,
                    guid: extra.guid.clone(),
                    vid: extra.vid,
                    pid: extra.pid,
                    serial: extra.serial.clone(),
                    annotation: ann,
                    canonical,
                    raw_port_type: raw_type,
                    manufacturer: extra.manufacturer.clone(),
                    product: extra.product.clone(),
                });
            }
            if let Ok(s) = serde_json::to_string_pretty(&out) {
                println!("{s}");
            } else {
                // Fallback to plain listing
                for (p, _) in ports_enriched.iter() {
                    println!("{p_port}", p_port = p.port_name);
                }
            }
        } else {
            for (p, _) in ports_enriched.iter() {
                println!("{p_port}", p_port = p.port_name);
            }
        }
        return true;
    }

    // Handle modbus slave listen
    if let Some(port) = matches.get_one::<String>("slave-listen") {
        if let Err(err) = super::modbus::slave::handle_slave_listen(matches, port).await {
            eprintln!("Error in slave-listen: {err}");
            std::process::exit(1);
        }
        return true;
    }

    // Handle modbus slave listen persist
    if let Some(port) = matches.get_one::<String>("slave-listen-persist") {
        // Check if IPC socket path is provided
        if let Some(ipc_socket_path) = matches.get_one::<String>("ipc-socket-path") {
            // Use IPC channel mode (half-duplex JSON request-response)
            if let Err(err) = super::modbus::slave::handle_slave_listen_ipc_channel(
                matches,
                port,
                ipc_socket_path,
            )
            .await
            {
                eprintln!("Error in slave-listen-persist (IPC channel mode): {err}");
                std::process::exit(1);
            }
        } else {
            // Use regular stdio mode
            if let Err(err) = super::modbus::slave::handle_slave_listen_persist(matches, port).await
            {
                eprintln!("Error in slave-listen-persist: {err}");
                std::process::exit(1);
            }
        }
        return true;
    }

    // Handle modbus slave poll (client mode - sends request)
    if let Some(port) = matches.get_one::<String>("slave-poll") {
        if let Err(err) = super::modbus::slave::handle_slave_poll(matches, port).await {
            eprintln!("Error in slave-poll: {err}");
            std::process::exit(1);
        }
        return true;
    }

    // Handle modbus slave poll persist (client mode - continuous polling)
    if let Some(port) = matches.get_one::<String>("slave-poll-persist") {
        if let Err(err) = super::modbus::slave::handle_slave_poll_persist(matches, port).await {
            eprintln!("Error in slave-poll-persist: {err}");
            std::process::exit(1);
        }
        return true;
    }

    // Handle modbus master provide
    if let Some(port) = matches.get_one::<String>("master-provide") {
        if let Err(err) = super::modbus::master::handle_master_provide(matches, port).await {
            eprintln!("Error in master-provide: {err}");
            std::process::exit(1);
        }
        return true;
    }

    // Handle modbus master provide persist
    if let Some(port) = matches.get_one::<String>("master-provide-persist") {
        if let Err(err) = super::modbus::master::handle_master_provide_persist(matches, port).await
        {
            eprintln!("Error in master-provide-persist: {err}");
            std::process::exit(1);
        }
        return true;
    }

    false
}

fn compute_canonical(name: &str) -> Option<String> {
    // Try to find COM<number> anywhere (case-insensitive)
    let up = name.to_uppercase();
    if let Some(pos) = up.find("COM") {
        let tail = &up[pos + 3..];
        let mut num = String::new();
        for c in tail.chars() {
            if c.is_ascii_digit() {
                num.push(c);
            } else {
                break;
            }
        }
        if !num.is_empty() {
            return Some(format!("COM{num}"));
        }
    }
    // Fallback: take basename after last '/'
    if let Some(b) = name.rsplit('/').next() {
        return Some(b.to_string());
    }
    None
}

/// Handle configuration mode
pub async fn handle_config_mode(matches: &ArgMatches) -> bool {
    // Handle configuration file
    if let Some(config_file) = matches.get_one::<String>("config") {
        println!("Loading configuration from file: {config_file}");
        match super::config::ModbusBootConfig::from_file(config_file) {
            Ok(config) => {
                println!(
                    "Configuration loaded successfully for port: {}",
                    config.port_name
                );
                // Start the ports defined in the configuration
                if let Err(err) = start_configuration(&config).await {
                    eprintln!("Error starting configuration: {err}");
                    std::process::exit(1);
                }
                println!("Configuration mode completed successfully");
                return true;
            }
            Err(err) => {
                eprintln!("Error loading configuration file: {err}");
                std::process::exit(1);
            }
        }
    }

    // Handle JSON configuration string
    if let Some(json_config) = matches.get_one::<String>("config-json") {
        println!("Loading configuration from JSON string");
        match super::config::ModbusBootConfig::from_json(json_config) {
            Ok(config) => {
                println!(
                    "Configuration loaded successfully for port: {}",
                    config.port_name
                );
                // Start the ports defined in the configuration
                if let Err(err) = start_configuration(&config).await {
                    eprintln!("Error starting configuration: {err}");
                    std::process::exit(1);
                }
                println!("Configuration mode completed successfully");
                return true;
            }
            Err(err) => {
                eprintln!("Error parsing JSON configuration: {err}");
                std::process::exit(1);
            }
        }
    }

    false
}

/// Start the ports defined in the configuration
async fn start_configuration(config: &super::config::ModbusBootConfig) -> anyhow::Result<()> {
    log::info!(
        "Starting port: {} with {} stations",
        config.port_name,
        config.stations.len()
    );

    // Start handlers for each station based on its mode
    for station in &config.stations {
        log::info!(
            "  - Station {}: mode={}, coils={}, discrete={}, holding={}, input={}",
            station.station_id,
            station.mode,
            station.map.coils.len(),
            station.map.discrete_inputs.len(),
            station.map.holding.len(),
            station.map.input.len()
        );

        // Log register ranges for this station
        for range in &station.map.coils {
            log::info!(
                "    Coils: addr={}, len={}",
                range.address_start,
                range.length
            );
        }
        for range in &station.map.discrete_inputs {
            log::info!(
                "    Discrete Inputs: addr={}, len={}",
                range.address_start,
                range.length
            );
        }
        for range in &station.map.holding {
            log::info!(
                "    Holding: addr={}, len={}",
                range.address_start,
                range.length
            );
        }
        for range in &station.map.input {
            log::info!(
                "    Input: addr={}, len={}",
                range.address_start,
                range.length
            );
        }
    }

    log::info!("Configuration started successfully");

    // Start the actual runtime with the config
    // Use global task manager to spawn the runtime
    let config_clone = config.clone();
    let task = spawn_task(async move { run_config_runtime(&config_clone).await });

    // Wait for the task to complete
    let _ = task.await;

    Ok(())
}

/// Run the configuration in an async runtime
async fn run_config_runtime(config: &super::config::ModbusBootConfig) -> anyhow::Result<()> {
    use rmodbus::server::context::ModbusContext;
    use std::io::Write;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // Open the serial port
    let port_handle = serialport::new(&config.port_name, config.baud_rate)
        .timeout(std::time::Duration::from_millis(100))
        .open()
        .map_err(|e| anyhow!("Failed to open port {}: {}", config.port_name, e))?;

    let port_arc = Arc::new(Mutex::new(port_handle));

    // Initialize storage for all stations
    let storage = std::sync::Arc::new(std::sync::Mutex::new(
        rmodbus::server::storage::ModbusStorageSmall::default(),
    ));

    // Populate initial values for master stations
    for station in &config.stations {
        if matches!(station.mode, super::config::StationMode::Master) {
            let mut storage_lock = storage.lock().unwrap();

            // Set initial values for coils
            for range in &station.map.coils {
                for (i, &val) in range.initial_values.iter().enumerate() {
                    let addr = range.address_start + i as u16;
                    if let Err(e) = storage_lock.set_coil(addr, val != 0) {
                        log::warn!("Failed to set coil at {addr}: {e}");
                    }
                }
            }

            // Set initial values for discrete inputs
            for range in &station.map.discrete_inputs {
                for (i, &val) in range.initial_values.iter().enumerate() {
                    let addr = range.address_start + i as u16;
                    if let Err(e) = storage_lock.set_discrete(addr, val != 0) {
                        log::warn!("Failed to set discrete input at {addr}: {e}");
                    }
                }
            }

            // Set initial values for holding registers
            for range in &station.map.holding {
                for (i, &val) in range.initial_values.iter().enumerate() {
                    let addr = range.address_start + i as u16;
                    if let Err(e) = storage_lock.set_holding(addr, val) {
                        log::warn!("Failed to set holding register at {addr}: {e}");
                    }
                }
            }

            // Set initial values for input registers
            for range in &station.map.input {
                for (i, &val) in range.initial_values.iter().enumerate() {
                    let addr = range.address_start + i as u16;
                    if let Err(e) = storage_lock.set_input(addr, val) {
                        log::warn!("Failed to set input register at {addr}: {e}");
                    }
                }
            }
        }
    }

    log::info!("Starting persistent Modbus server/client loop for config mode");

    // Run the main loop
    loop {
        // Process Modbus frames
        let mut buffer = [0u8; 256];
        let mut port = port_arc.lock().await;

        match port.read(&mut buffer) {
            Ok(n) if n > 0 => {
                drop(port); // Release lock before processing

                // Process the frame
                let request = &buffer[..n];
                if let Some(response) = process_modbus_frame(request, &storage, &config.stations) {
                    let mut port = port_arc.lock().await;
                    if let Err(e) = port.write_all(&response) {
                        log::error!("Failed to write response: {e}");
                    }
                }
            }
            Ok(_) => {
                // No data, sleep briefly
                drop(port);
                sleep_1s().await;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // Timeout is expected, just continue
                drop(port);
                sleep_1s().await;
            }
            Err(e) => {
                log::error!("Error reading from port: {e}");
                drop(port);
                sleep_1s().await;
            }
        }
    }
}

/// Process a Modbus frame and generate a response
fn process_modbus_frame(
    request: &[u8],
    storage: &std::sync::Arc<std::sync::Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
    stations: &[super::config::StationConfig],
) -> Option<Vec<u8>> {
    use rmodbus::{server::ModbusFrame, ModbusProto};

    if request.len() < 2 {
        return None;
    }

    let station_id = request[0];

    // Find if we have a station with this ID configured as slave/listener
    let has_station = stations
        .iter()
        .any(|s| s.station_id == station_id && matches!(s.mode, super::config::StationMode::Slave));

    if !has_station {
        // Not our station, ignore
        return None;
    }

    // Process the request using rmodbus
    let storage_lock = storage.lock().unwrap();

    // Parse and respond to the request
    let mut response = Vec::new();
    let mut frame = ModbusFrame::new(station_id, request, ModbusProto::Rtu, &mut response);

    if let Err(e) = frame.parse() {
        log::warn!("Failed to parse Modbus frame: {e:?}");
        return None;
    }

    if let Err(e) = frame.process_read(&*storage_lock) {
        log::warn!("Failed to process Modbus read: {e:?}");
        return None;
    }

    drop(storage_lock);

    if response.is_empty() {
        None
    } else {
        Some(response)
    }
}
