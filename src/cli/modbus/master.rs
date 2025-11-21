use anyhow::{anyhow, Result};
#[cfg(unix)]
use std::path::PathBuf;
use std::{
    cell::RefCell,
    collections::{hash_map::DefaultHasher, HashMap},
    hash::Hasher,
    io::{BufRead, BufReader, Read, Write},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{extract::State, http::StatusCode, Json, Router};
use clap::ArgMatches;
#[cfg(unix)]
use interprocess::local_socket::GenericFilePath;
use interprocess::local_socket::{prelude::*, GenericNamespaced, ListenerOptions, Name};
use rmodbus::{
    server::{context::ModbusContext, storage::ModbusStorageSmall},
    ModbusProto,
};

use super::{
    emit_modbus_ipc_log, extract_values_from_station_configs, parse_data_line, parse_register_mode,
    ModbusIpcLogPayload,
};
use crate::api::{modbus::ModbusResponse, utils::open_serial_port};

#[derive(Clone)]
pub enum DataSource {
    Manual,
    File(String),
    Pipe(String),
    MqttServer(String), // URL
    HttpServer(u16),    // Port
    IpcPipe(String),    // pipe path
}

impl std::str::FromStr for DataSource {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "manual" {
            Ok(DataSource::Manual)
        } else if let Some(path) = s.strip_prefix("file:") {
            Ok(DataSource::File(path.to_string()))
        } else if let Some(name) = s.strip_prefix("pipe:") {
            Ok(DataSource::Pipe(name.to_string()))
        } else if let Some(url) = s.strip_prefix("mqtt://") {
            Ok(DataSource::MqttServer(format!("mqtt://{}", url)))
        } else if let Some(url) = s.strip_prefix("mqtts://") {
            Ok(DataSource::MqttServer(format!("mqtts://{}", url)))
        } else if let Some(port_str) = s.strip_prefix("http://") {
            let port: u16 = port_str
                .parse()
                .map_err(|_| anyhow!("Invalid HTTP port number"))?;
            Ok(DataSource::HttpServer(port))
        } else if let Some(path) = s.strip_prefix("ipc:") {
            Ok(DataSource::IpcPipe(path.to_string()))
        } else {
            Err(anyhow!("Invalid data source format"))
        }
    }
}

use crate::{
    cli::{actions, cleanup, http_daemon_registry as http_registry},
    core::task_manager::spawn_task,
    protocol::{
        modbus::{
            build_slave_coils_response, build_slave_discrete_inputs_response,
            build_slave_holdings_response, build_slave_inputs_response,
        },
        status::crc16_modbus,
    },
};

const SERIAL_PORT_OPEN_RETRIES: usize = 3;

async fn open_serial_port_with_retry(
    port: &str,
    baud_rate: u32,
    timeout: Duration,
) -> Result<Box<dyn serialport::SerialPort>> {
    let mut last_error = String::new();
    for attempt in 1..=SERIAL_PORT_OPEN_RETRIES {
        log::info!(
            "Attempting to open serial port {port} (attempt {attempt}/{SERIAL_PORT_OPEN_RETRIES})"
        );
        match open_serial_port(port, baud_rate, timeout) {
            Ok(handle) => {
                if attempt > 1 {
                    log::info!("Opened serial port {port} after {attempt} attempts");
                }
                return Ok(handle);
            }
            Err(err) => {
                last_error = err.to_string();
                if attempt < SERIAL_PORT_OPEN_RETRIES {
                    log::warn!(
                        "Failed to open serial port {port} (attempt {attempt}/{SERIAL_PORT_OPEN_RETRIES}): {last_error}"
                    );
                    sleep_1s().await;
                }
            }
        }
    }

    Err(anyhow!(
        "Failed to open port {port} after {SERIAL_PORT_OPEN_RETRIES} attempts: {last_error}"
    ))
}

/// Shared state for axum HTTP server
#[derive(Clone)]
struct HttpServerState {
    tx: flume::Sender<Vec<crate::protocol::status::types::modbus::StationConfig>>,
    storage: Option<Arc<Mutex<ModbusStorageSmall>>>,
    /// Track the most recent station configurations received via POST
    stations: Arc<Mutex<Vec<crate::protocol::status::types::modbus::StationConfig>>>,
}

use crate::protocol::status::types::modbus::StationConfig as ProtocolStationConfig;
use crate::protocol::status::types::modbus::StationsResponse;
use crate::utils::sleep::{sleep_1s, sleep_3s};

/// Axum handler for GET / endpoint - retrieve all station data from storage
async fn handle_stations_get(
    State(state): State<HttpServerState>,
) -> Result<(StatusCode, Json<StationsResponse>), (StatusCode, String)> {
    log::info!("📤 HTTP Server: Received GET request for all station data");

    // Get the currently configured stations
    let configured_stations = state.stations.lock().unwrap().clone();

    let stations_snapshot: Vec<ProtocolStationConfig> = if let Some(ref storage) = state.storage {
        // Build snapshots with current values from storage for each configured station
        let mut snapshots = Vec::new();

        for station in &configured_stations {
            match crate::cli::modbus::build_station_snapshot_from_storage(storage, station) {
                Ok(snapshot) => snapshots.push(snapshot),
                Err(e) => {
                    log::warn!(
                        "Failed to build snapshot for station {}: {e}",
                        station.station_id
                    );
                    // Fall back to returning the configuration without current values
                    snapshots.push(station.clone());
                }
            }
        }

        snapshots
    } else {
        // No storage available - return configured stations as-is
        configured_stations
    };

    let resp = StationsResponse {
        success: true,
        message: format!("Retrieved {} stations", stations_snapshot.len()),
        stations: stations_snapshot,
    };

    Ok((StatusCode::OK, Json(resp)))
}

/// Axum handler for POST /stations endpoint
async fn handle_stations_post(
    State(state): State<HttpServerState>,
    Json(stations): Json<Vec<crate::protocol::status::types::modbus::StationConfig>>,
) -> Result<(StatusCode, Json<StationsResponse>), (StatusCode, String)> {
    // helper functions live in `crate::cli::modbus`

    log::info!(
        "📥 HTTP Server: Received POST request with {} stations",
        stations.len()
    );
    log::debug!("📥 HTTP Server: Station details: {:?}", stations);

    // Clone stations for forwarding and for building the response snapshot
    let stations_for_send = stations.clone();

    // Update the tracked stations
    {
        let mut tracked = state.stations.lock().unwrap();
        *tracked = stations.clone();
    }

    // Forward stations to the update thread
    state.tx.send_async(stations_for_send).await.map_err(|e| {
        log::error!("Failed to send stations to update thread: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Internal Server Error: {}", e),
        )
    })?;

    // Attempt to read current values from storage (if available). We poll
    // briefly (up to 3s) for the update thread to apply the changes.
    let mut stations_snapshot: Vec<ProtocolStationConfig> = Vec::new();

    if let Some(ref storage) = state.storage {
        let start_wait = Instant::now();
        let timeout = Duration::from_secs(3);

        // Try to read updated values; if storage hasn't been populated yet,
        // keep retrying until timeout.
        loop {
            stations_snapshot.clear();
            let mut ok = true;
            for station in &stations {
                match crate::cli::modbus::build_station_snapshot_from_storage(storage, station) {
                    Ok(sc) => stations_snapshot.push(sc),
                    Err(_) => {
                        ok = false;
                        break;
                    }
                }
            }

            if ok || start_wait.elapsed() >= timeout {
                break;
            }

            sleep_1s().await;
        }
    } else {
        // No storage available — fall back to echoing posted initial values
        for station in &stations {
            stations_snapshot.push(station.clone());
        }
    }

    let resp = StationsResponse {
        success: true,
        message: "Stations queued".to_string(),
        stations: stations_snapshot,
    };

    Ok((StatusCode::OK, Json(resp)))
}

/// Run HTTP server daemon using axum
async fn run_http_server_daemon(
    port: u16,
    tx: flume::Sender<Vec<crate::protocol::status::types::modbus::StationConfig>>,
    shutdown_rx: flume::Receiver<()>,
    storage: Option<Arc<Mutex<ModbusStorageSmall>>>,
) -> Result<()> {
    let addr = format!("127.0.0.1:{}", port);
    log::info!("Starting HTTP server daemon on {}", addr);

    let state = HttpServerState {
        tx,
        storage,
        stations: Arc::new(Mutex::new(Vec::new())),
    };

    // Build axum router with GET and POST endpoints
    let app = Router::new()
        .route(
            "/",
            axum::routing::get(handle_stations_get).post(handle_stations_post),
        )
        .with_state(state);

    // Use task_manager to spawn the async HTTP server daemon
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| anyhow!("Failed to bind HTTP server to {}: {}", addr, e))?;

    log::info!("HTTP server daemon listening on {}", addr);

    // Create shutdown signal from channel
    let shutdown_signal = async move {
        match shutdown_rx.recv_async().await {
            Ok(()) => {
                log::info!("HTTP server daemon received shutdown signal, exiting");
            }
            Err(_) => {
                log::info!("HTTP server shutdown channel closed, exiting");
            }
        }
    };

    // Run axum server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await
        .map_err(|e| anyhow!("HTTP server error: {}", e))?;

    Ok(())
}

/// Handle master provide (temporary: output once and exit)
pub async fn handle_master_provide(matches: &ArgMatches, port: &str) -> Result<()> {
    let station_id = *matches.get_one::<u8>("station-id").unwrap();
    let register_address = *matches.get_one::<u16>("register-address").unwrap();
    let register_length = *matches.get_one::<u16>("register-length").unwrap();
    let register_mode = matches.get_one::<String>("register-mode").unwrap();
    let baud_rate = *matches.get_one::<u32>("baud-rate").unwrap();
    let data_source_str = matches
        .get_one::<String>("data-source")
        .ok_or_else(|| anyhow!("--data-source is required for master mode"))?;

    let reg_mode = parse_register_mode(register_mode)?;
    let data_source = data_source_str.parse::<DataSource>()?;

    log::info!(
        "Starting master provide on {port} (station_id={station_id}, addr={register_address}, len={register_length}, mode={reg_mode:?}, baud={baud_rate})"
    );

    // Start HTTP server daemon if using HTTP data source. Register the join
    // handle in a global registry so other code (mode switches) can shut it
    // down later.
    let (http_tx, http_rx) =
        flume::unbounded::<Vec<crate::protocol::status::types::modbus::StationConfig>>();
    let _http_server_thread_port = if let DataSource::HttpServer(http_port) = &data_source {
        let port = *http_port;
        let tx = http_tx.clone();
        let (shutdown_tx, shutdown_rx) = flume::bounded::<()>(1);
        let handle = spawn_task(async move {
            run_http_server_daemon(port, tx, shutdown_rx, None).await?;
            Ok(())
        });

        // register handle+shutdown sender in global registry for lookup/shutdown
        let _arc = http_registry::register_handle(port, handle, shutdown_tx.clone());

        // Ensure cleanup on program exit also shuts down the daemon
        cleanup::register_cleanup(move || {
            std::mem::drop(http_registry::shutdown_and_join(port));
        });

        Some(port)
    } else {
        None
    };

    // Read one line of data
    let values = read_one_data_update(
        &data_source,
        station_id,
        reg_mode,
        register_address,
        register_length,
    )
    .await?;

    // Initialize modbus storage with values
    use rmodbus::server::storage::ModbusStorageSmall;
    let storage = Arc::new(Mutex::new(ModbusStorageSmall::default()));
    let storage_clone = storage.clone();
    {
        let mut context = storage.lock().unwrap();
        match reg_mode {
            crate::protocol::status::types::modbus::RegisterMode::Holding => {
                for (i, &val) in values.iter().enumerate() {
                    context.set_holding(register_address + i as u16, val)?;
                }
            }
            crate::protocol::status::types::modbus::RegisterMode::Coils => {
                for (i, &val) in values.iter().enumerate() {
                    context.set_coil(register_address + i as u16, val != 0)?;
                }
            }
            crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
                for (i, &val) in values.iter().enumerate() {
                    context.set_discrete(register_address + i as u16, val != 0)?;
                }
            }
            crate::protocol::status::types::modbus::RegisterMode::Input => {
                for (i, &val) in values.iter().enumerate() {
                    context.set_input(register_address + i as u16, val)?;
                }
            }
        }
    }

    // If HTTP server mode, check for incoming data before serial port opens
    if matches!(&data_source, DataSource::HttpServer(_)) {
        // Wait for HTTP POST data (with timeout)
        match http_rx.recv_timeout(Duration::from_secs(15)) {
            Ok(stations) => {
                log::info!("Received HTTP POST with {} stations", stations.len());
                // Update storage with received data
                let mut context = storage_clone.lock().unwrap();
                for station in &stations {
                    if station.station_id == station_id {
                        // Update holding registers
                        for range in &station.map.holding {
                            for (i, &val) in range.initial_values.iter().enumerate() {
                                let addr = range.address_start + i as u16;
                                context.set_holding(addr, val)?;
                            }
                        }
                        // Update coils
                        for range in &station.map.coils {
                            for (i, &val) in range.initial_values.iter().enumerate() {
                                let addr = range.address_start + i as u16;
                                context.set_coil(addr, val != 0)?;
                            }
                        }
                        // Update discrete inputs
                        for range in &station.map.discrete_inputs {
                            for (i, &val) in range.initial_values.iter().enumerate() {
                                let addr = range.address_start + i as u16;
                                context.set_discrete(addr, val != 0)?;
                            }
                        }
                        // Update input registers
                        for range in &station.map.input {
                            for (i, &val) in range.initial_values.iter().enumerate() {
                                let addr = range.address_start + i as u16;
                                context.set_input(addr, val)?;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log::warn!("Timeout waiting for HTTP POST data: {}", e);
            }
        }
    }

    // Open serial port and wait for one request, then respond and exit
    let port_handle = open_serial_port_with_retry(port, baud_rate, Duration::from_secs(5)).await?;

    let port_arc = Arc::new(Mutex::new(Some(port_handle)));

    // Wait for request and respond once
    let mut buffer = [0u8; 256];
    let mut assembling: Vec<u8> = Vec::new();
    let start_time = std::time::Instant::now();

    loop {
        if start_time.elapsed() > Duration::from_secs(10) {
            return Err(anyhow!("Timeout waiting for request"));
        }

        enum ReadAction {
            Data,
            FrameReady,
            Timeout,
            Error(String),
            NoData,
        }

        let action = {
            let mut port_lock = port_arc.lock().unwrap();
            if let Some(port) = port_lock.as_mut() {
                match port.read(&mut buffer) {
                    Ok(n) if n > 0 => {
                        assembling.extend_from_slice(&buffer[..n]);
                        ReadAction::Data
                    }
                    Ok(_) => {
                        if !assembling.is_empty() {
                            ReadAction::FrameReady
                        } else {
                            ReadAction::NoData
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                        if !assembling.is_empty() {
                            ReadAction::FrameReady
                        } else {
                            ReadAction::Timeout
                        }
                    }
                    Err(err) => ReadAction::Error(err.to_string()),
                }
            } else {
                ReadAction::Error("Port is None".to_string())
            }
        };

        match action {
            ReadAction::Data => {
                sleep_1s().await;
            }
            ReadAction::FrameReady => {
                let request = assembling.clone();
                let (response, _) = respond_to_request(
                    port_arc.clone(),
                    &request,
                    station_id,
                    &storage,
                    &mut None,
                    "",
                )?;
                let json = serde_json::to_string(&response)?;
                println!("{json}");
                drop(port_arc);
                sleep_1s().await;
                return Ok(());
            }
            ReadAction::Timeout => {
                sleep_1s().await;
            }
            ReadAction::Error(e) => {
                log::warn!("Error reading from port: {e}");
                sleep_1s().await;
                return Err(anyhow!("Error reading from port: {e}"));
            }
            ReadAction::NoData => {
                // nothing to do
            }
        }
    }
}

/// Handle master provide persist (continuous JSONL output)
/// Master mode acts as Modbus Slave/Server - listens for requests and responds with data
pub async fn handle_master_provide_persist(matches: &ArgMatches, port: &str) -> Result<()> {
    let station_id = *matches.get_one::<u8>("station-id").unwrap();
    let register_address = *matches.get_one::<u16>("register-address").unwrap();
    let register_length = *matches.get_one::<u16>("register-length").unwrap();
    let register_mode = matches.get_one::<String>("register-mode").unwrap();
    let baud_rate = *matches.get_one::<u32>("baud-rate").unwrap();
    let data_source_str = matches
        .get_one::<String>("data-source")
        .ok_or_else(|| anyhow!("--data-source is required for master mode"))?;

    let reg_mode = parse_register_mode(register_mode)?;
    let data_source = data_source_str.parse::<DataSource>()?;
    let port_name = port;

    log::info!(
        "Starting persistent master provide on {port} (station_id={station_id}, addr={register_address}, len={register_length}, mode={reg_mode:?}, baud={baud_rate})"
    );
    log::info!("Master mode: acting as Modbus Slave/Server - listening for requests and responding with data");

    // Setup IPC if requested
    let mut ipc_connections = actions::setup_ipc(matches);

    // Check if debug CI E2E test mode is enabled
    let _debug_dump_thread = if matches.get_flag("debug-ci-e2e-test") {
        log::info!("🔍 Debug CI E2E test mode enabled for CLI subprocess");

        let port_name = port.to_string();
        let station_id_copy = station_id;
        let reg_mode_copy = reg_mode;
        let register_address_copy = register_address;
        let register_length_copy = register_length;

        // Extract basename from port path (e.g., "/tmp/vcom1" -> "vcom1")
        let port_basename = std::path::Path::new(&port)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(port);
        let dump_path =
            std::path::PathBuf::from(format!("/tmp/ci_cli_{port_basename}_status.json"));

        Some(
            crate::protocol::status::debug_dump::start_status_dump_thread(
                dump_path,
                None,
                std::sync::Arc::new(move || {
                    crate::protocol::status::types::cli::CliStatus::new_master_provide(
                        port_name.clone(),
                        station_id_copy,
                        reg_mode_copy,
                        register_address_copy,
                        register_length_copy,
                    )
                    .to_json()
                }),
            ),
        )
    } else {
        None
    };

    // Check if this is a virtual port (IPC/HTTP) that doesn't need a physical serial port
    let is_virtual = crate::protocol::modbus::is_virtual_port(port);

    // Open serial port with longer timeout for reading requests (only for physical ports)
    let port_arc = if is_virtual {
        log::info!("Port {port} is a virtual port (IPC/HTTP) - skipping physical serial port open");

        // Notify IPC that virtual port is ready
        if let Some(ref mut ipc_conns) = ipc_connections {
            let _ = ipc_conns
                .status
                .send(&crate::protocol::ipc::IpcMessage::PortOpened {
                    port_name: port.to_string(),
                    timestamp: None,
                });
            log::info!("IPC: Sent PortOpened message for virtual port {port}");
        }

        // Return None wrapped in Arc<Mutex<>> for virtual ports
        Arc::new(Mutex::new(None))
    } else {
        let port_handle =
            match open_serial_port_with_retry(port, baud_rate, Duration::from_millis(50)).await {
                Ok(handle) => handle,
                Err(err) => {
                    if let Some(ref mut ipc_conns) = ipc_connections {
                        let _ =
                            ipc_conns
                                .status
                                .send(&crate::protocol::ipc::IpcMessage::PortError {
                                    port_name: port.to_string(),
                                    error: format!("Failed to open port: {err}"),
                                    timestamp: None,
                                });
                    }
                    return Err(err);
                }
            };

        // Notify IPC that port was opened successfully
        if let Some(ref mut ipc_conns) = ipc_connections {
            let _ = ipc_conns
                .status
                .send(&crate::protocol::ipc::IpcMessage::PortOpened {
                    port_name: port.to_string(),
                    timestamp: None,
                });
            log::info!("IPC: Sent PortOpened message for physical port {port}");
        }

        Arc::new(Mutex::new(Some(port_handle)))
    };

    // Register cleanup to ensure port is released on program exit
    {
        let pa = port_arc.clone();
        cleanup::register_cleanup(move || {
            // Drop the Arc to release the port
            drop(pa);
        });
    }

    // Initialize modbus storage with values from data source
    use rmodbus::server::storage::ModbusStorageSmall;
    let storage = Arc::new(Mutex::new(ModbusStorageSmall::default()));

    // Load initial data into storage
    let initial_values = read_one_data_update(
        &data_source,
        station_id,
        reg_mode,
        register_address,
        register_length,
    )
    .await?;
    log::info!("Loaded initial values: {initial_values:?}");
    {
        let mut context = storage.lock().unwrap();
        match reg_mode {
            crate::protocol::status::types::modbus::RegisterMode::Holding => {
                for (i, &val) in initial_values.iter().enumerate() {
                    context.set_holding(register_address + i as u16, val)?;
                }
            }
            crate::protocol::status::types::modbus::RegisterMode::Coils => {
                for (i, &val) in initial_values.iter().enumerate() {
                    context.set_coil(register_address + i as u16, val != 0)?;
                }
            }
            crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
                for (i, &val) in initial_values.iter().enumerate() {
                    context.set_discrete(register_address + i as u16, val != 0)?;
                }
            }
            crate::protocol::status::types::modbus::RegisterMode::Input => {
                for (i, &val) in initial_values.iter().enumerate() {
                    context.set_input(register_address + i as u16, val)?;
                }
            }
        }
    }

    // Start a background thread to update storage with new values from data source
    // For pipe data sources we spawn the background updater; for file data sources
    // the updater will still be spawned but printing of JSON to stdout is
    // de-duplicated below to avoid repeated identical log lines when polled
    let storage_clone = storage.clone();
    let data_source_clone = data_source.clone();

    // Track recent changed ranges so the main loop can bypass debounce when a
    // request overlaps a recently-updated register range.
    let changed_ranges: Arc<Mutex<Vec<(u16, u16, Instant)>>> = Arc::new(Mutex::new(Vec::new()));
    let changed_ranges_clone = changed_ranges.clone();

    // Create HTTP server daemon if needed. Keep handle in shared container and
    // register cleanup so it can be shutdown/joined on cleanup.
    let (http_tx, http_rx) =
        flume::unbounded::<Vec<crate::protocol::status::types::modbus::StationConfig>>();
    let http_server_thread: Option<u16> = if let DataSource::HttpServer(port) = &data_source {
        let port = *port;
        let tx = http_tx.clone();
        let (shutdown_tx, shutdown_rx) = flume::bounded::<()>(1);
        let storage_for_thread = storage_clone.clone();
        let handle = spawn_task(async move {
            run_http_server_daemon(port, tx, shutdown_rx, Some(storage_for_thread)).await?;
            Ok(())
        });

        // Register handle+shutdown sender into global registry
        let _arc = http_registry::register_handle(port, handle, shutdown_tx.clone());

        // ensure cleanup will shutdown and join
        cleanup::register_cleanup(move || {
            std::mem::drop(http_registry::shutdown_and_join(port));
        });

        Some(port)
    } else {
        None
    };

    let http_rx_clone = if matches!(&data_source, DataSource::HttpServer(_)) {
        Some(http_rx.clone())
    } else {
        None
    };

    // Create IPC socket server if --ipc-socket-path is provided
    if let Some(ipc_socket_path) = matches.get_one::<String>("ipc-socket-path") {
        log::info!("🔌 IPC: Starting IPC socket server at {}", ipc_socket_path);

        let ipc_socket_path = ipc_socket_path.clone();
        let ipc_storage = storage.clone();
        let ipc_station_id = station_id;
        let ipc_reg_mode = reg_mode;
        let ipc_register_address = register_address;
        let ipc_register_length = register_length;
        let ipc_changed_ranges = changed_ranges.clone();

        // Spawn IPC server in a blocking thread since interprocess accept() is blocking
        let _ipc_handle = tokio::task::spawn_blocking(move || {
            log::info!("🔌 IPC: Task spawned, calling run_ipc_socket_server");
            match run_ipc_socket_server_sync(
                &ipc_socket_path,
                ipc_storage,
                ipc_station_id,
                ipc_reg_mode,
                ipc_register_address,
                ipc_register_length,
                ipc_changed_ranges,
            ) {
                Ok(()) => {
                    log::info!("🔌 IPC: Socket server ended normally");
                }
                Err(e) => {
                    log::error!("🔌 IPC: Socket server error: {}", e);
                }
            }
        });
    }

    let update_args = UpdateStorageArgs {
        storage: storage_clone,
        data_source: data_source_clone,
        station_id,
        reg_mode,
        register_address,
        register_length,
        changed_ranges: changed_ranges_clone,
        http_rx: http_rx_clone,
    };

    let _update_thread = spawn_task(async move { update_storage_loop(update_args).await });

    // Parse optional debounce seconds argument (floating seconds). Default 1.0s
    // Single-precision seconds argument
    let debounce_seconds = matches
        .get_one::<f32>("debounce-seconds")
        .copied()
        .unwrap_or(1.0_f32);

    // Printing/de-duplication state
    // We track by a key derived from the request bytes + response values to
    // handle two duplicate scenarios:
    // 1) The same request arrives multiple times in a short window -> debounce
    // 2) Different requests produce the same values -> dedupe by values
    // Use RefCell for interior mutability so the closure doesn't capture a
    // long-lived mutable borrow of these maps and block other borrows.
    let last_print_times: RefCell<HashMap<u64, Instant>> = RefCell::new(HashMap::new());
    let last_values_by_key: RefCell<HashMap<u64, Vec<u16>>> = RefCell::new(HashMap::new());

    // Debounce window: if same request key printed within this duration, skip
    // Convert floating seconds to Duration (support fractional seconds)
    let debounce_window = if debounce_seconds <= 0.0 {
        Duration::from_secs(0)
    } else {
        let ms = (debounce_seconds * 1000.0).round() as u64;
        Duration::from_millis(ms)
    };

    // TTL for stale cache entries (so the maps don't grow forever). Use a
    // multiple of debounce_window; if debounce_window is zero, use 10s default.
    let cache_ttl = if debounce_window == Duration::from_secs(0) {
        Duration::from_secs(10)
    } else {
        debounce_window * 4
    };

    // Helper to optionally print response JSON while handling duplicate suppression
    // Uses a key (hash) which should be derived from the original request bytes
    // so repeated identical requests within the debounce window won't spam stdout.
    let print_response = |request_key: u64, response: &ModbusResponse, force: bool| -> Result<()> {
        let now = Instant::now();

        // If force flag is set, bypass debounce and emit immediately
        if force {
            let json = serde_json::to_string(response)?;
            println!("{json}");
            last_values_by_key
                .borrow_mut()
                .insert(request_key, response.values.clone());
            last_print_times.borrow_mut().insert(request_key, now);
            return Ok(());
        }

        // If values are identical to last printed for this key, skip
        if let Some(prev_vals) = last_values_by_key.borrow().get(&request_key) {
            if prev_vals == &response.values {
                // Update last print time to extend debounce even if we don't print
                last_print_times.borrow_mut().insert(request_key, now);
                return Ok(());
            }
        }

        // If we printed something for this key recently, skip printing (debounce)
        if let Some(last) = last_print_times.borrow().get(&request_key) {
            if now.duration_since(*last) < debounce_window {
                // Update stored values and time, but do not emit
                last_values_by_key
                    .borrow_mut()
                    .insert(request_key, response.values.clone());
                last_print_times.borrow_mut().insert(request_key, now);
                return Ok(());
            }
        }

        // Otherwise emit JSON and record time/values
        let json = serde_json::to_string(response)?;
        println!("{json}");
        last_values_by_key
            .borrow_mut()
            .insert(request_key, response.values.clone());
        last_print_times.borrow_mut().insert(request_key, now);
        Ok(())
    };

    // Main loop: listen for requests and respond
    let mut buffer = [0u8; 256];
    let mut assembling: Vec<u8> = Vec::new();
    let mut last_byte_time: Option<std::time::Instant> = None;
    let frame_gap = Duration::from_millis(10); // Inter-frame gap

    log::info!("CLI Master: Entering main loop, listening for requests on {port}");

    loop {
        // Check if HTTP server thread has panicked
        if let Some(port) = http_server_thread {
            if let Some(is_finished) = http_registry::is_handle_finished(port) {
                if is_finished {
                    return Err(anyhow!("HTTP server thread terminated unexpectedly"));
                }
            }
        }

        // Accept command connection if not yet connected
        if let Some(ref mut ipc_conns) = ipc_connections {
            // Try to accept command connection - retry on each loop iteration until successful
            static COMMAND_ACCEPTED: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(false);
            // Track if we've received the first StationsUpdate (initial configuration)
            static FIRST_CONFIG_RECEIVED: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(false);

            if !COMMAND_ACCEPTED.load(std::sync::atomic::Ordering::Relaxed) {
                match ipc_conns.command_listener.accept() {
                    Ok(()) => {
                        log::info!("Command channel accepted");
                        COMMAND_ACCEPTED.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    Err(e) => {
                        // Don't log every attempt to avoid spam, just keep trying
                        log::trace!("Command channel accept not ready yet: {e}");
                    }
                }
            }

            // Check for incoming commands
            if COMMAND_ACCEPTED.load(std::sync::atomic::Ordering::Relaxed) {
                if let Ok(Some(msg)) = ipc_conns.command_listener.try_recv() {
                    match msg {
                        crate::protocol::ipc::IpcMessage::StationsUpdate {
                            stations_data,
                            update_reason,
                            ..
                        } => {
                            let is_first_config =
                                !FIRST_CONFIG_RECEIVED.load(std::sync::atomic::Ordering::Relaxed);
                            FIRST_CONFIG_RECEIVED.store(true, std::sync::atomic::Ordering::Relaxed);

                            log::info!(
                                "Received stations update ({} bytes, is_first={}, reason={:?})",
                                stations_data.len(),
                                is_first_config,
                                update_reason
                            );

                            // Deserialize stations using postcard
                            if let Ok(stations) = postcard::from_bytes::<
                                Vec<crate::cli::config::StationConfig>,
                            >(&stations_data)
                            {
                                log::info!("Deserialized {} stations", stations.len());

                                // Determine whether to accept zero values based on update reason
                                // - "user_edit": Accept all values including 0 (user intention)
                                // - "initial_config": Skip 0 values (avoid overwriting with defaults)
                                // - "sync" or None: Skip 0 values on first config only
                                // - "read_response": Always apply (actual modbus read data)
                                let allow_zero_writes = match update_reason.as_deref() {
                                    Some("user_edit") => true,
                                    Some("read_response") => true,
                                    Some("initial_config") => false,
                                    Some("sync") | None => !is_first_config, // Legacy behavior
                                    Some(other) => {
                                        log::warn!(
                                            "Unknown update_reason: {other}, using legacy behavior"
                                        );
                                        !is_first_config
                                    }
                                };

                                log::info!(
                                    "Master storage decision: reason={:?}, is_first={}, allow_zeros={}",
                                    update_reason,
                                    is_first_config,
                                    allow_zero_writes
                                );

                                // Apply the station updates to storage
                                let mut context = storage.lock().unwrap();
                                for station in &stations {
                                    log::info!(
                                        "  Applying Station {}: mode={:?}",
                                        station.station_id,
                                        station.mode
                                    );

                                    // Update holding registers
                                    for range in &station.map.holding {
                                        log::info!(
                                            "🔧 Processing holding range: addr=0x{:04X}, len={}, values={:?}, allow_zeros={}",
                                            range.address_start,
                                            range.initial_values.len(),
                                            range.initial_values,
                                            allow_zero_writes
                                        );
                                        for (i, &val) in range.initial_values.iter().enumerate() {
                                            let addr = range.address_start + i as u16;
                                            // Apply based on update reason
                                            if allow_zero_writes || val != 0 {
                                                if let Err(e) = context.set_holding(addr, val) {
                                                    log::warn!(
                                                        "❌ Failed to set holding register at 0x{addr:04X}: {e}"
                                                    );
                                                } else {
                                                    log::info!("✅ Updated holding register 0x{addr:04X} = 0x{val:04X} (reason={:?})", update_reason);
                                                }
                                            } else {
                                                log::info!("⏭️  Skipped holding register 0x{addr:04X} (value=0, reason={:?})", update_reason);
                                            }
                                        }
                                    }

                                    // Update coils
                                    for range in &station.map.coils {
                                        for (i, &val) in range.initial_values.iter().enumerate() {
                                            let addr = range.address_start + i as u16;
                                            // Apply based on update reason
                                            if allow_zero_writes || val != 0 {
                                                if let Err(e) = context.set_coil(addr, val != 0) {
                                                    log::warn!("Failed to set coil at {addr}: {e}");
                                                } else {
                                                    log::info!(
                                                        "✏️ Updated coil 0x{addr:04X} = {val} (reason={:?})",
                                                        update_reason
                                                    );
                                                }
                                            } else {
                                                log::info!("⏭️  Skipped coil 0x{addr:04X} (value=0, reason={:?})", update_reason);
                                            }
                                        }
                                    }

                                    // Update discrete inputs
                                    for range in &station.map.discrete_inputs {
                                        for (i, &val) in range.initial_values.iter().enumerate() {
                                            let addr = range.address_start + i as u16;
                                            // Apply based on update reason
                                            if allow_zero_writes || val != 0 {
                                                if let Err(e) = context.set_discrete(addr, val != 0)
                                                {
                                                    log::warn!(
                                                        "Failed to set discrete input at {addr}: {e}"
                                                    );
                                                } else {
                                                    log::info!("✏️ Updated discrete input 0x{addr:04X} = {val} (reason={:?})", update_reason);
                                                }
                                            } else {
                                                log::info!("⏭️  Skipped discrete input 0x{addr:04X} (value=0, reason={:?})", update_reason);
                                            }
                                        }
                                    }

                                    // Update input registers
                                    for range in &station.map.input {
                                        for (i, &val) in range.initial_values.iter().enumerate() {
                                            let addr = range.address_start + i as u16;
                                            // Apply based on update reason
                                            if allow_zero_writes || val != 0 {
                                                if let Err(e) = context.set_input(addr, val) {
                                                    log::warn!(
                                                        "Failed to set input register at {addr}: {e}"
                                                    );
                                                } else {
                                                    log::info!("✏️ Updated input register 0x{addr:04X} = 0x{val:04X} (reason={:?})", update_reason);
                                                }
                                            } else {
                                                log::info!("⏭️  Skipped input register 0x{addr:04X} (value=0, reason={:?})", update_reason);
                                            }
                                        }
                                    }
                                }
                                log::info!("Applied all station updates to storage");
                            } else {
                                log::warn!("Failed to deserialize stations data");
                            }
                        }
                        _ => {
                            log::debug!("Ignoring non-command IPC message");
                        }
                    }
                }
            }
        }

        // Cleanup stale entries from the print caches on each loop iteration
        // to prevent unbounded growth. We remove entries older than cache_ttl.
        if !last_print_times.borrow().is_empty() {
            let now = Instant::now();
            // Collect expired keys first (avoid holding an immutable borrow while mutating)
            let expired: Vec<u64> = last_print_times
                .borrow()
                .iter()
                .filter_map(|(k, &t)| {
                    if now.duration_since(t) > cache_ttl {
                        Some(*k)
                    } else {
                        None
                    }
                })
                .collect();
            for k in expired {
                last_print_times.borrow_mut().remove(&k);
                last_values_by_key.borrow_mut().remove(&k);
            }
        }

        enum ReadAction2 {
            Data,
            FrameReady(
                Option<(
                    u16,
                    u16,
                    crate::protocol::status::types::modbus::RegisterMode,
                )>,
            ),
            Timeout,
            Error(String),
            NoData,
        }

        let action2 = {
            // First check if the port is virtual without holding the lock across an await.
            let is_virtual = {
                let port = port_arc.lock().unwrap();
                port.is_none()
            };

            // For virtual ports, skip serial port reading entirely
            if is_virtual {
                // Virtual port: just sleep a bit to avoid busy loop
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                ReadAction2::NoData
            } else {
                let mut port = port_arc.lock().unwrap();
                let result = match port.as_mut().unwrap().read(&mut buffer) {
                    Ok(n) if n > 0 => {
                        log::info!(
                            "CLI Master: Read {n} bytes from port: {:02X?}",
                            &buffer[..n]
                        );
                        assembling.extend_from_slice(&buffer[..n]);
                        last_byte_time = Some(std::time::Instant::now());
                        ReadAction2::Data
                    }
                    Ok(_) => {
                        if !assembling.is_empty() {
                            if let Some(last_time) = last_byte_time {
                                if last_time.elapsed() >= frame_gap {
                                    // Determine parsed_range without holding lock
                                    let request_preview = assembling.clone();
                                    let parsed_range = if request_preview.len() >= 8 {
                                        let func = request_preview[1];
                                        match func {
                                            0x01 => {
                                                let start = u16::from_be_bytes([
                                                    request_preview[2],
                                                    request_preview[3],
                                                ]);
                                                let qty = u16::from_be_bytes([
                                                    request_preview[4],
                                                    request_preview[5],
                                                ]);
                                                Some((start, qty, crate::protocol::status::types::modbus::RegisterMode::Coils))
                                            }
                                            0x02 => {
                                                let start = u16::from_be_bytes([
                                                    request_preview[2],
                                                    request_preview[3],
                                                ]);
                                                let qty = u16::from_be_bytes([
                                                    request_preview[4],
                                                    request_preview[5],
                                                ]);
                                                Some((start, qty, crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs))
                                            }
                                            0x03 => {
                                                let start = u16::from_be_bytes([
                                                    request_preview[2],
                                                    request_preview[3],
                                                ]);
                                                let qty = u16::from_be_bytes([
                                                    request_preview[4],
                                                    request_preview[5],
                                                ]);
                                                Some((start, qty, crate::protocol::status::types::modbus::RegisterMode::Holding))
                                            }
                                            0x04 => {
                                                let start = u16::from_be_bytes([
                                                    request_preview[2],
                                                    request_preview[3],
                                                ]);
                                                let qty = u16::from_be_bytes([
                                                    request_preview[4],
                                                    request_preview[5],
                                                ]);
                                                Some((start, qty, crate::protocol::status::types::modbus::RegisterMode::Input))
                                            }
                                            _ => None,
                                        }
                                    } else {
                                        None
                                    };
                                    ReadAction2::FrameReady(parsed_range)
                                } else {
                                    ReadAction2::NoData
                                }
                            } else {
                                ReadAction2::NoData
                            }
                        } else {
                            ReadAction2::NoData
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                        if !assembling.is_empty() {
                            if let Some(last_time) = last_byte_time {
                                if last_time.elapsed() >= frame_gap {
                                    let request_preview = assembling.clone();
                                    let parsed_range = if request_preview.len() >= 8 {
                                        let func = request_preview[1];
                                        match func {
                                            0x01 => {
                                                let start = u16::from_be_bytes([
                                                    request_preview[2],
                                                    request_preview[3],
                                                ]);
                                                let qty = u16::from_be_bytes([
                                                    request_preview[4],
                                                    request_preview[5],
                                                ]);
                                                Some((start, qty, crate::protocol::status::types::modbus::RegisterMode::Coils))
                                            }
                                            0x02 => {
                                                let start = u16::from_be_bytes([
                                                    request_preview[2],
                                                    request_preview[3],
                                                ]);
                                                let qty = u16::from_be_bytes([
                                                    request_preview[4],
                                                    request_preview[5],
                                                ]);
                                                Some((start, qty, crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs))
                                            }
                                            0x03 => {
                                                let start = u16::from_be_bytes([
                                                    request_preview[2],
                                                    request_preview[3],
                                                ]);
                                                let qty = u16::from_be_bytes([
                                                    request_preview[4],
                                                    request_preview[5],
                                                ]);
                                                Some((start, qty, crate::protocol::status::types::modbus::RegisterMode::Holding))
                                            }
                                            0x04 => {
                                                let start = u16::from_be_bytes([
                                                    request_preview[2],
                                                    request_preview[3],
                                                ]);
                                                let qty = u16::from_be_bytes([
                                                    request_preview[4],
                                                    request_preview[5],
                                                ]);
                                                Some((start, qty, crate::protocol::status::types::modbus::RegisterMode::Input))
                                            }
                                            _ => None,
                                        }
                                    } else {
                                        None
                                    };
                                    ReadAction2::FrameReady(parsed_range)
                                } else {
                                    ReadAction2::Timeout
                                }
                            } else {
                                ReadAction2::Timeout
                            }
                        } else {
                            ReadAction2::Timeout
                        }
                    }
                    Err(err) => ReadAction2::Error(err.to_string()),
                };
                result
            }
        };

        match action2 {
            ReadAction2::Data => {}
            ReadAction2::FrameReady(parsed_range) => {
                log::info!(
                    "CLI Master: Frame complete ({} bytes), processing request",
                    assembling.len()
                );
                let request = assembling.clone();
                assembling.clear();
                last_byte_time = None;

                match respond_to_request(
                    port_arc.clone(),
                    &request,
                    station_id,
                    &storage,
                    &mut ipc_connections,
                    port_name,
                ) {
                    Ok((response, response_frame)) => {
                        let mut hasher = DefaultHasher::new();
                        hasher.write(&request);
                        let request_key = hasher.finish();

                        // Determine overlap with recent changes
                        let mut force = false;
                        if let Some((start, qty, _mode)) = parsed_range {
                            let now = Instant::now();
                            let cr = changed_ranges.lock().unwrap();
                            for (cstart, clen, t) in cr.iter() {
                                if now.duration_since(*t) > cache_ttl {
                                    continue;
                                }
                                let a1 = start as u32;
                                let a2 = (start + qty) as u32;
                                let b1 = *cstart as u32;
                                let b2 = (cstart + clen) as u32;
                                if a1 < b2 && b1 < a2 {
                                    force = true;
                                    break;
                                }
                            }
                        }

                        if let Err(e) = print_response(request_key, &response, force) {
                            log::warn!("Failed to print response: {e}");
                        }

                        emit_modbus_ipc_log(
                            &mut ipc_connections,
                            ModbusIpcLogPayload {
                                port: port_name,
                                direction: "tx",
                                frame: &response_frame,
                                station_id: Some(response.station_id),
                                register_mode: parsed_range.map(|(_, _, mode)| mode),
                                start_address: parsed_range.map(|(start, _, _)| start),
                                quantity: parsed_range.map(|(_, qty, _)| qty),
                                success: Some(true),
                                error: None,
                                config_index: None,
                            },
                        );
                    }
                    Err(err) => {
                        log::warn!("Error responding to request: {err}");
                        emit_modbus_ipc_log(
                            &mut ipc_connections,
                            ModbusIpcLogPayload {
                                port: port_name,
                                direction: "tx",
                                frame: &request,
                                station_id: request.first().copied(),
                                register_mode: parsed_range.map(|(_, _, mode)| mode),
                                start_address: parsed_range.map(|(start, _, _)| start),
                                quantity: parsed_range.map(|(_, qty, _)| qty),
                                success: Some(false),
                                error: Some(format!("{err:#}")),
                                config_index: None,
                            },
                        );
                    }
                }
            }
            ReadAction2::Timeout => {}
            ReadAction2::Error(e) => {
                log::warn!("CLI Master read error: {e}");
                sleep_1s().await;
                continue;
            }
            ReadAction2::NoData => {}
        }
        // No explicit drop needed here; ensure we don't mistakenly drop the `port` parameter (a &str)

        // Small sleep to avoid busy loop
        sleep_1s().await;
    }
}

/// Respond to a Modbus request (acting as Slave/Server)
fn respond_to_request(
    port_arc: Arc<Mutex<Option<Box<dyn serialport::SerialPort>>>>,
    request: &[u8],
    station_id: u8,
    storage: &Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
    ipc: &mut Option<crate::cli::actions::IpcConnections>,
    _port_name: &str,
) -> Result<(ModbusResponse, Vec<u8>)> {
    use rmodbus::server::ModbusFrame;

    if request.len() < 2 {
        log::warn!(
            "respond_to_request: Request too short (len={})",
            request.len()
        );
        return Err(anyhow!("Request too short"));
    }

    let request_station_id = request[0];
    if request_station_id != station_id {
        log::debug!(
            "respond_to_request: Ignoring request for station {request_station_id} (we are station {station_id})",
        );
        return Err(anyhow!(
            "Request for different station ID: {request_station_id} (we are {station_id})",
        ));
    }

    log::info!("respond_to_request: Received request from slave: {request:02X?}");

    // Parse and respond to request
    let mut context = storage.lock().unwrap();
    let mut response_buf = Vec::new();
    let mut frame = ModbusFrame::new(station_id, request, ModbusProto::Rtu, &mut response_buf);
    frame.parse()?;

    log::debug!(
        "respond_to_request: Parsed frame - func={:?}, reg_addr=0x{:04X?}, count={}",
        frame.func,
        frame.reg,
        frame.count
    );

    #[allow(unreachable_patterns)]
    let response = match frame.func {
        rmodbus::consts::ModbusFunction::GetHoldings => {
            match build_slave_holdings_response(&mut frame, &mut context) {
                Ok(Some(resp)) => {
                    log::debug!(
                        "respond_to_request: Built holdings response ({} bytes)",
                        resp.len()
                    );
                    resp
                }
                _ => {
                    log::error!("respond_to_request: Failed to build holdings response");
                    return Err(anyhow!("Failed to build holdings response"));
                }
            }
        }
        rmodbus::consts::ModbusFunction::GetInputs => {
            match build_slave_inputs_response(&mut frame, &mut context) {
                Ok(Some(resp)) => {
                    log::debug!(
                        "respond_to_request: Built input registers response ({} bytes)",
                        resp.len()
                    );
                    resp
                }
                _ => {
                    log::error!("respond_to_request: Failed to build input registers response");
                    return Err(anyhow!("Failed to build input registers response"));
                }
            }
        }
        rmodbus::consts::ModbusFunction::GetCoils => {
            match build_slave_coils_response(&mut frame, &mut context) {
                Ok(Some(resp)) => {
                    log::debug!(
                        "respond_to_request: Built coils response ({} bytes)",
                        resp.len()
                    );
                    resp
                }
                _ => {
                    log::error!("respond_to_request: Failed to build coils response");
                    return Err(anyhow!("Failed to build coils response"));
                }
            }
        }
        rmodbus::consts::ModbusFunction::GetDiscretes => {
            match build_slave_discrete_inputs_response(&mut frame, &mut context) {
                Ok(Some(resp)) => {
                    log::debug!(
                        "respond_to_request: Built discrete inputs response ({} bytes)",
                        resp.len()
                    );
                    resp
                }
                _ => {
                    log::error!("respond_to_request: Failed to build discrete inputs response");
                    return Err(anyhow!("Failed to build discrete inputs response"));
                }
            }
        }
        // Write function codes - Modbus allows slaves to write to master's registers
        // This is used when slave device needs to update values in the master
        rmodbus::consts::ModbusFunction::SetHolding => {
            // 0x06 - Write Single Holding Register
            if request.len() < 6 {
                log::error!("respond_to_request: SetHolding request too short");
                return Err(anyhow!("SetHolding request too short"));
            }
            let write_addr = u16::from_be_bytes([request[2], request[3]]);
            let write_value = u16::from_be_bytes([request[4], request[5]]);
            log::info!(
                "respond_to_request: Write Single Holding Register - addr=0x{:04X}, value=0x{:04X}",
                write_addr,
                write_value
            );

            // Write to storage
            log::info!(
                "📝 Master: Writing to storage addr=0x{write_addr:04X} value=0x{write_value:04X}"
            );
            context.set_holdings_bulk(write_addr, &[write_value])?;
            log::info!("✅ Master: Storage updated successfully");

            // Read back the updated value to send in StationsUpdate
            let updated_value = context.get_holding(write_addr)?;

            // Notify TUI via IPC with StationsUpdate
            // Note: Master doesn't send RegisterWriteComplete because it didn't initiate the write
            // The slave CLI will send RegisterWriteComplete to its own TUI after receiving response
            if let Some(ref mut ipc_conns) = ipc {
                // Send StationsUpdate to reflect the new value in master TUI immediately
                let station_config = crate::cli::config::StationConfig::single_range(
                    station_id,
                    crate::cli::config::StationMode::Master,
                    crate::protocol::status::types::modbus::RegisterMode::Holding,
                    write_addr,
                    1, // Single register
                    Some(vec![updated_value]),
                );

                match postcard::to_allocvec(&vec![station_config]) {
                    Ok(stations_data) => {
                        let msg = crate::protocol::ipc::IpcMessage::stations_update(stations_data);
                        if let Err(e) = ipc_conns.status.send(&msg) {
                            log::warn!("Failed to send StationsUpdate via IPC: {e}");
                        } else {
                            log::info!("📤 Master: Sent StationsUpdate IPC message addr=0x{write_addr:04X} value=0x{updated_value:04X}");
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to serialize StationConfig: {e}");
                    }
                }
            }

            // Echo back the request as response for function 0x06
            request.to_vec()
        }
        rmodbus::consts::ModbusFunction::SetHoldingsBulk => {
            // 0x10 - Write Multiple Holding Registers
            if request.len() < 7 {
                log::error!("respond_to_request: SetHoldingBulk request too short");
                return Err(anyhow!("SetHoldingBulk request too short"));
            }
            let write_addr = u16::from_be_bytes([request[2], request[3]]);
            let write_count = u16::from_be_bytes([request[4], request[5]]);
            let byte_count = request[6] as usize;

            if request.len() < 7 + byte_count {
                log::error!("respond_to_request: SetHoldingBulk request incomplete");
                return Err(anyhow!("SetHoldingBulk request incomplete"));
            }

            let mut values = Vec::new();
            for i in 0..write_count {
                let offset = 7 + (i as usize * 2);
                let value = u16::from_be_bytes([request[offset], request[offset + 1]]);
                values.push(value);
            }

            log::info!(
                "respond_to_request: Write Multiple Holding Registers - addr=0x{:04X}, count={}, values={:?}",
                write_addr,
                write_count,
                values
            );

            // Write to storage
            context.set_holdings_bulk(write_addr, &values)?;

            // Notify master TUI via StationsUpdate
            if let Some(ref mut ipc_conns) = ipc {
                let station_config = crate::cli::config::StationConfig::single_range(
                    station_id,
                    crate::cli::config::StationMode::Master,
                    crate::protocol::status::types::modbus::RegisterMode::Holding,
                    write_addr,
                    write_count,
                    Some(values.clone()),
                );

                match postcard::to_allocvec(&vec![station_config]) {
                    Ok(stations_data) => {
                        let msg = crate::protocol::ipc::IpcMessage::stations_update(stations_data);
                        if let Err(e) = ipc_conns.status.send(&msg) {
                            log::warn!("Failed to send StationsUpdate via IPC: {e}");
                        } else {
                            log::info!("📤 Master: Sent StationsUpdate for {} holdings starting at addr=0x{write_addr:04X}", write_count);
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to serialize StationConfig: {e}");
                    }
                }
            }

            // Response for 0x10: [station_id, func, addr_hi, addr_lo, count_hi, count_lo, crc_lo, crc_hi]
            let mut resp = vec![
                station_id, 0x10, request[2], request[3], request[4], request[5],
            ];
            let crc = crc16_modbus(&resp);
            resp.push((crc & 0xFF) as u8);
            resp.push(((crc >> 8) & 0xFF) as u8);
            resp
        }
        rmodbus::consts::ModbusFunction::SetCoil => {
            // 0x05 - Write Single Coil
            if request.len() < 6 {
                log::error!("respond_to_request: SetCoil request too short");
                return Err(anyhow!("SetCoil request too short"));
            }
            let write_addr = u16::from_be_bytes([request[2], request[3]]);
            let write_value = u16::from_be_bytes([request[4], request[5]]);
            let coil_state = write_value == 0xFF00;
            log::info!(
                "respond_to_request: Write Single Coil - addr=0x{:04X}, value={}",
                write_addr,
                coil_state
            );

            // Write to storage
            let coil_bytes = [if coil_state { 0xFF } else { 0x00 }];
            context.set_coils_from_u8(write_addr, 1, &coil_bytes)?;

            // Notify master TUI via StationsUpdate
            if let Some(ref mut ipc_conns) = ipc {
                let station_config = crate::cli::config::StationConfig::single_range(
                    station_id,
                    crate::cli::config::StationMode::Master,
                    crate::protocol::status::types::modbus::RegisterMode::Coils,
                    write_addr,
                    1,
                    Some(vec![if coil_state { 1 } else { 0 }]),
                );

                match postcard::to_allocvec(&vec![station_config]) {
                    Ok(stations_data) => {
                        let msg = crate::protocol::ipc::IpcMessage::stations_update(stations_data);
                        if let Err(e) = ipc_conns.status.send(&msg) {
                            log::warn!("Failed to send StationsUpdate via IPC: {e}");
                        } else {
                            log::info!("📤 Master: Sent StationsUpdate for coil addr=0x{write_addr:04X} state={coil_state}");
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to serialize StationConfig: {e}");
                    }
                }
            }

            // Echo back the request as response for function 0x05
            request.to_vec()
        }
        rmodbus::consts::ModbusFunction::SetCoilsBulk => {
            // 0x0F - Write Multiple Coils
            if request.len() < 7 {
                log::error!("respond_to_request: SetCoilBulk request too short");
                return Err(anyhow!("SetCoilBulk request too short"));
            }
            let write_addr = u16::from_be_bytes([request[2], request[3]]);
            let write_count = u16::from_be_bytes([request[4], request[5]]);
            let byte_count = request[6] as usize;

            if request.len() < 7 + byte_count {
                log::error!("respond_to_request: SetCoilBulk request incomplete");
                return Err(anyhow!("SetCoilBulk request incomplete"));
            }

            log::info!(
                "respond_to_request: Write Multiple Coils - addr=0x{:04X}, count={}",
                write_addr,
                write_count
            );

            // Write to storage
            context.set_coils_from_u8(write_addr, write_count, &request[7..7 + byte_count])?;

            // Notify master TUI via StationsUpdate
            if let Some(ref mut ipc_conns) = ipc {
                // Extract coil values from the write request
                let mut coil_values = Vec::new();
                for i in 0..write_count {
                    let byte_idx = (i / 8) as usize;
                    let bit_idx = (i % 8) as usize;
                    let coil_value = if byte_idx < byte_count {
                        if (request[7 + byte_idx] & (1 << bit_idx)) != 0 {
                            1
                        } else {
                            0
                        }
                    } else {
                        0
                    };
                    coil_values.push(coil_value);
                }

                let station_config = crate::cli::config::StationConfig::single_range(
                    station_id,
                    crate::cli::config::StationMode::Master,
                    crate::protocol::status::types::modbus::RegisterMode::Coils,
                    write_addr,
                    write_count,
                    Some(coil_values),
                );

                match postcard::to_allocvec(&vec![station_config]) {
                    Ok(stations_data) => {
                        let msg = crate::protocol::ipc::IpcMessage::stations_update(stations_data);
                        if let Err(e) = ipc_conns.status.send(&msg) {
                            log::warn!("Failed to send StationsUpdate via IPC: {e}");
                        } else {
                            log::info!("📤 Master: Sent StationsUpdate for {} coils starting at addr=0x{write_addr:04X}", write_count);
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to serialize StationConfig: {e}");
                    }
                }
            }

            // Response for 0x0F: [station_id, func, addr_hi, addr_lo, count_hi, count_lo, crc_lo, crc_hi]
            let mut resp = vec![
                station_id, 0x0F, request[2], request[3], request[4], request[5],
            ];
            let crc = crc16_modbus(&resp);
            resp.push((crc & 0xFF) as u8);
            resp.push(((crc >> 8) & 0xFF) as u8);
            resp
        }
        _ => {
            log::error!(
                "respond_to_request: Unsupported function code: {:?}",
                frame.func
            );
            return Err(anyhow!("Unsupported function code: {:?}", frame.func));
        }
    };

    drop(context);

    // Send response (only for physical ports, virtual ports don't need serial write)
    let response_frame = response.clone();
    {
        let mut port_guard = port_arc.lock().unwrap();
        if let Some(ref mut port) = *port_guard {
            port.write_all(&response)?;
            port.flush()?;
            log::info!("respond_to_request: Sent response via physical serial port");
        } else {
            log::info!(
                "respond_to_request: Virtual port - response ready but no physical write needed"
            );
        }
    }

    log::info!("respond_to_request: Sent response to slave: {response:02X?}");

    // Extract values from response for JSON output
    let values = extract_values_from_response(&response)?;
    log::debug!("respond_to_request: Extracted values for output: {values:?}");

    let register_mode_label = match frame.func {
        rmodbus::consts::ModbusFunction::GetHoldings => "holding",
        rmodbus::consts::ModbusFunction::GetCoils => "coils",
        rmodbus::consts::ModbusFunction::GetDiscretes => "discrete",
        rmodbus::consts::ModbusFunction::GetInputs => "input",
        _ => "unknown",
    };

    Ok((
        ModbusResponse {
            station_id,
            register_address: frame.reg,
            register_mode: register_mode_label.to_string(),
            values,
            timestamp: chrono::Utc::now().to_rfc3339(),
        },
        response_frame,
    ))
}

/// Arguments for the update storage loop.
struct UpdateStorageArgs {
    storage: Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
    data_source: DataSource,
    station_id: u8,
    reg_mode: crate::protocol::status::types::modbus::RegisterMode,
    register_address: u16,
    register_length: u16,
    changed_ranges: Arc<Mutex<Vec<(u16, u16, Instant)>>>,
    http_rx: Option<flume::Receiver<Vec<crate::protocol::status::types::modbus::StationConfig>>>,
}

/// Update storage loop - continuously reads data from source and updates storage
async fn update_storage_loop(args: UpdateStorageArgs) -> Result<()> {
    let UpdateStorageArgs {
        storage,
        data_source,
        station_id,
        reg_mode,
        register_address,
        register_length,
        changed_ranges,
        http_rx,
    } = args;
    loop {
        match &data_source {
            DataSource::Manual => {
                // Manual mode: no automatic updates, values are set via IPC or other means
                log::debug!("Manual data source mode - sleeping");
                sleep_3s().await;
                continue;
            }
            DataSource::MqttServer(url) => {
                // MQTT: subscribe to broker and continuously update on new messages
                log::info!("Starting MQTT subscription loop for: {}", url);

                // Parse MQTT URL
                let parsed_url = match url::Url::parse(url) {
                    Ok(u) => u,
                    Err(e) => {
                        log::error!("Invalid MQTT URL: {}", e);
                        return Err(anyhow!("Invalid MQTT URL: {}", e));
                    }
                };

                let host = parsed_url.host_str().unwrap_or("localhost").to_string();
                let port = parsed_url.port().unwrap_or(1883);
                let topic = parsed_url.path().trim_start_matches('/').to_string();

                if topic.is_empty() {
                    log::error!("MQTT URL must include a topic path");
                    return Err(anyhow!("MQTT URL must include a topic path"));
                }

                log::info!("MQTT: connecting to {}:{}, topic: {}", host, port, topic);

                // Create channel to receive MQTT messages from blocking thread
                let (mqtt_tx, mqtt_rx) = flume::unbounded::<String>();

                // Spawn blocking task for MQTT connection
                let mqtt_task = tokio::task::spawn_blocking(move || {
                    // Create a unique client ID
                    let client_id = format!("aoba_{}", uuid::Uuid::new_v4());

                    // Create MQTT options
                    let mqtt_options = rumqttc::MqttOptions::new(&client_id, host, port);

                    // Create client
                    let (client, mut connection) = rumqttc::Client::new(mqtt_options, 10);

                    // Subscribe to topic
                    if let Err(e) = client.subscribe(&topic, rumqttc::QoS::AtMostOnce) {
                        log::error!("Failed to subscribe to MQTT topic: {}", e);
                        return Err(anyhow!("Failed to subscribe: {}", e));
                    }

                    log::info!("MQTT: subscribed to topic '{}'", topic);

                    // Process incoming messages and send via channel
                    for notification in connection.iter() {
                        match notification {
                            Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(publish))) => {
                                let payload = String::from_utf8_lossy(&publish.payload).to_string();
                                log::debug!("Received MQTT message: {}", payload);

                                // Send to async loop
                                if mqtt_tx.send(payload).is_err() {
                                    log::warn!("MQTT receiver dropped, stopping");
                                    break;
                                }
                            }
                            Ok(_) => {
                                // Other events, ignore
                            }
                            Err(e) => {
                                log::warn!("MQTT connection error: {}", e);
                                break;
                            }
                        }
                    }

                    log::warn!("MQTT connection closed");
                    Ok::<(), anyhow::Error>(())
                });

                // Receive MQTT messages and update storage
                loop {
                    match mqtt_rx.recv_timeout(Duration::from_secs(1)) {
                        Ok(payload) => {
                            log::info!(
                                "📥 MQTT: Received message payload (len={}): {}",
                                payload.len(),
                                payload
                            );
                            if let Ok(values) = parse_data_line(
                                &payload,
                                station_id,
                                reg_mode,
                                register_address,
                                register_length,
                            ) {
                                log::info!(
                                    "✅ MQTT: Updating storage with {} values from MQTT: {:?}",
                                    values.len(),
                                    values
                                );
                                {
                                    let mut context = storage.lock().unwrap();
                                    match reg_mode {
                                        crate::protocol::status::types::modbus::RegisterMode::Holding => {
                                            for (i, &val) in values.iter().enumerate() {
                                                context.set_holding(register_address + i as u16, val)?;
                                            }
                                        }
                                        crate::protocol::status::types::modbus::RegisterMode::Coils => {
                                            for (i, &val) in values.iter().enumerate() {
                                                context.set_coil(register_address + i as u16, val != 0)?;
                                            }
                                        }
                                        crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
                                            for (i, &val) in values.iter().enumerate() {
                                                context.set_discrete(register_address + i as u16, val != 0)?;
                                            }
                                        }
                                        crate::protocol::status::types::modbus::RegisterMode::Input => {
                                            for (i, &val) in values.iter().enumerate() {
                                                context.set_input(register_address + i as u16, val)?;
                                            }
                                        }
                                    }
                                }

                                // Record changed range
                                {
                                    let len = values.len() as u16;
                                    let mut cr = changed_ranges.lock().unwrap();
                                    cr.push((register_address, len, Instant::now()));
                                    while cr.len() > 1000 {
                                        cr.remove(0);
                                    }
                                }
                            } else {
                                log::warn!("Failed to parse MQTT message data");
                            }
                        }
                        Err(flume::RecvTimeoutError::Timeout) => {
                            // Check if blocking task finished
                            if mqtt_task.is_finished() {
                                log::warn!("MQTT task finished, reconnecting...");
                                sleep_3s().await;
                                break;
                            }
                            // Continue waiting
                            continue;
                        }
                        Err(flume::RecvTimeoutError::Disconnected) => {
                            log::warn!("MQTT connection lost, will retry...");
                            sleep_3s().await;
                            break;
                        }
                    }
                }
            }
            DataSource::HttpServer(_port) => {
                // HTTP Server: receive data from HTTP daemon via channel
                let rx = http_rx
                    .as_ref()
                    .ok_or_else(|| anyhow!("HTTP receiver channel not initialized"))?;

                log::info!("HTTP Server: waiting for data from HTTP daemon...");

                loop {
                    match rx.recv_timeout(Duration::from_secs(1)) {
                        Ok(stations) => {
                            log::info!(
                                "📥 HTTP: Received {} stations from HTTP server",
                                stations.len()
                            );
                            log::debug!("📥 HTTP: Station data: {:?}", stations);

                            // Extract values for this station
                            match super::extract_values_from_station_configs(
                                &stations,
                                station_id,
                                reg_mode,
                                register_address,
                                register_length,
                            ) {
                                Ok(values) => {
                                    log::info!(
                                        "✅ HTTP: Updating storage with {} values from HTTP server: {:?}",
                                        values.len(),
                                        values
                                    );
                                    {
                                        let mut context = storage.lock().unwrap();
                                        match reg_mode {
                                            crate::protocol::status::types::modbus::RegisterMode::Holding => {
                                                for (i, &val) in values.iter().enumerate() {
                                                    context.set_holding(register_address + i as u16, val)?;
                                                }
                                            }
                                            crate::protocol::status::types::modbus::RegisterMode::Coils => {
                                                for (i, &val) in values.iter().enumerate() {
                                                    context.set_coil(register_address + i as u16, val != 0)?;
                                                }
                                            }
                                            crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
                                                for (i, &val) in values.iter().enumerate() {
                                                    context.set_discrete(register_address + i as u16, val != 0)?;
                                                }
                                            }
                                            crate::protocol::status::types::modbus::RegisterMode::Input => {
                                                for (i, &val) in values.iter().enumerate() {
                                                    context.set_input(register_address + i as u16, val)?;
                                                }
                                            }
                                        }
                                    }

                                    // Record changed range
                                    {
                                        let len = values.len() as u16;
                                        let mut cr = changed_ranges.lock().unwrap();
                                        cr.push((register_address, len, Instant::now()));
                                        while cr.len() > 1000 {
                                            cr.remove(0);
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("Failed to extract values from HTTP data: {}", e);
                                }
                            }
                        }
                        Err(flume::RecvTimeoutError::Timeout) => {
                            // Timeout is normal, just continue
                            continue;
                        }
                        Err(flume::RecvTimeoutError::Disconnected) => {
                            log::error!("HTTP server channel disconnected");
                            return Err(anyhow!("HTTP server channel disconnected"));
                        }
                    }
                }
            }
            DataSource::IpcPipe(path) => {
                // IPC pipe: similar to regular Pipe
                log::info!("📂 IPC: Opening IPC pipe at: {}", path);
                let file = std::fs::File::open(path)?;
                let reader = BufReader::new(file);

                for line in reader.lines() {
                    let line = line?;
                    if line.trim().is_empty() {
                        continue;
                    }

                    log::info!("📥 IPC: Received line (len={}): {}", line.len(), line);
                    match parse_data_line(
                        &line,
                        station_id,
                        reg_mode,
                        register_address,
                        register_length,
                    ) {
                        Ok(values) => {
                            log::info!("✅ IPC: Updating storage with values from IPC: {values:?}");
                            {
                                let mut context = storage.lock().unwrap();
                                match reg_mode {
                                            crate::protocol::status::types::modbus::RegisterMode::Holding => {
                                                for (i, &val) in values.iter().enumerate() {
                                                    context.set_holding(register_address + i as u16, val)?;
                                                }
                                            }
                                            crate::protocol::status::types::modbus::RegisterMode::Coils => {
                                                for (i, &val) in values.iter().enumerate() {
                                                    context.set_coil(register_address + i as u16, val != 0)?;
                                                }
                                            }
                                            crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
                                                for (i, &val) in values.iter().enumerate() {
                                                    context.set_discrete(register_address + i as u16, val != 0)?;
                                                }
                                            }
                                            crate::protocol::status::types::modbus::RegisterMode::Input => {
                                                for (i, &val) in values.iter().enumerate() {
                                                    context.set_input(register_address + i as u16, val)?;
                                                }
                                            }
                                        }
                            }

                            // Record changed range
                            {
                                let len = values.len() as u16;
                                let mut cr = changed_ranges.lock().unwrap();
                                cr.push((register_address, len, Instant::now()));
                                while cr.len() > 1000 {
                                    cr.remove(0);
                                }
                            }

                            sleep_1s().await;
                        }
                        Err(err) => {
                            log::warn!("Error parsing data line from IPC: {err}");
                        }
                    }
                }

                // Pipe closed, reopen and continue
                log::debug!("IPC pipe closed, reopening...");
            }
            DataSource::File(path) => {
                // Try to open the file with better error handling
                log::info!("📂 File: Opening file data source at: {}", path);
                let file = match std::fs::File::open(path) {
                    Ok(f) => f,
                    Err(err) => {
                        log::error!("Failed to open data source file {path}: {err}");
                        log::error!("Update thread will exit, causing main process to terminate");
                        return Err(anyhow!("Failed to open data source file: {err}"));
                    }
                };
                let reader = BufReader::new(file);

                let mut line_count = 0;
                for line in reader.lines() {
                    let line = match line {
                        Ok(l) => l,
                        Err(err) => {
                            log::error!("Failed to read line from data source: {err}");
                            return Err(anyhow!("Failed to read line: {err}"));
                        }
                    };

                    if line.trim().is_empty() {
                        continue;
                    }

                    line_count += 1;
                    log::info!(
                        "📥 File: Reading line {} (len={}): {}",
                        line_count,
                        line.len(),
                        line
                    );
                    match parse_data_line(
                        &line,
                        station_id,
                        reg_mode,
                        register_address,
                        register_length,
                    ) {
                        Ok(values) => {
                            log::info!(
                                "✅ File: Updating storage with {} values from line {}: {:?}",
                                values.len(),
                                line_count,
                                values
                            );
                            {
                                let mut context = storage.lock().unwrap();
                                match reg_mode {
                                    crate::protocol::status::types::modbus::RegisterMode::Holding => {
                                        for (i, &val) in values.iter().enumerate() {
                                            context.set_holding(register_address + i as u16, val)?;
                                        }
                                    }
                                    crate::protocol::status::types::modbus::RegisterMode::Coils => {
                                        for (i, &val) in values.iter().enumerate() {
                                            context.set_coil(register_address + i as u16, val != 0)?;
                                        }
                                    }
                                    crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
                                        for (i, &val) in values.iter().enumerate() {
                                            context.set_discrete(register_address + i as u16, val != 0)?;
                                        }
                                    }
                                    crate::protocol::status::types::modbus::RegisterMode::Input => {
                                        for (i, &val) in values.iter().enumerate() {
                                            context.set_input(register_address + i as u16, val)?;
                                        }
                                    }
                                }
                            }

                            // Record changed range for other thread to detect overlap
                            {
                                let len = values.len() as u16;
                                let mut cr = changed_ranges.lock().unwrap();
                                cr.push((register_address, len, Instant::now()));
                                // Keep size bounded: trim old entries
                                while cr.len() > 1000 {
                                    cr.remove(0);
                                }
                            }

                            // Wait a bit before next update to avoid overwhelming
                            sleep_1s().await;
                        }
                        Err(err) => {
                            log::warn!("Error parsing data line {line_count}: {err}");
                        }
                    }
                }

                // After reading all lines, loop back to start of file
                log::debug!(
                    "Reached end of data file ({line_count} lines processed), looping back to start"
                );
            }
            DataSource::Pipe(path) => {
                // Open named pipe (FIFO) and continuously read from it
                log::info!("📂 Pipe: Opening named pipe at: {}", path);
                let file = std::fs::File::open(path)?;
                let reader = BufReader::new(file);

                for line in reader.lines() {
                    let line = line?;
                    if line.trim().is_empty() {
                        continue;
                    }

                    log::info!("📥 Pipe: Received line (len={}): {}", line.len(), line);
                    match parse_data_line(
                        &line,
                        station_id,
                        reg_mode,
                        register_address,
                        register_length,
                    ) {
                        Ok(values) => {
                            log::info!("✅ Pipe: Updating storage with values: {values:?}");
                            {
                                let mut context = storage.lock().unwrap();
                                match reg_mode {
                                    crate::protocol::status::types::modbus::RegisterMode::Holding => {
                                        for (i, &val) in values.iter().enumerate() {
                                            context.set_holding(register_address + i as u16, val)?;
                                        }
                                    }
                                    crate::protocol::status::types::modbus::RegisterMode::Coils => {
                                        for (i, &val) in values.iter().enumerate() {
                                            context.set_coil(register_address + i as u16, val != 0)?;
                                        }
                                    }
                                    crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
                                        for (i, &val) in values.iter().enumerate() {
                                            context.set_discrete(register_address + i as u16, val != 0)?;
                                        }
                                    }
                                    crate::protocol::status::types::modbus::RegisterMode::Input => {
                                        for (i, &val) in values.iter().enumerate() {
                                            context.set_input(register_address + i as u16, val)?;
                                        }
                                    }
                                }
                            }

                            // Record changed range for other thread to detect overlap
                            {
                                let len = values.len() as u16;
                                let mut cr = changed_ranges.lock().unwrap();
                                cr.push((register_address, len, Instant::now()));
                                while cr.len() > 1000 {
                                    cr.remove(0);
                                }
                            }

                            // Wait a bit before next update
                            sleep_1s().await;
                        }
                        Err(err) => {
                            log::warn!("Error parsing data line: {err}");
                        }
                    }
                }

                // Pipe closed by writer, reopen and continue
                log::debug!("Pipe closed, reopening...");
            }
        }
    }
}

/// Extract values from a Modbus response frame
fn extract_values_from_response(response: &[u8]) -> Result<Vec<u16>> {
    if response.len() < 3 {
        return Ok(vec![]);
    }

    let _station_id = response[0];
    let function_code = response[1];
    let byte_count = response[2] as usize;

    match function_code {
        0x03 | 0x04 => {
            // Read register response (holding or input registers)
            if response.len() < 3 + byte_count {
                return Err(anyhow!("Response too short for register data"));
            }
            let mut values = Vec::new();
            for i in (0..byte_count).step_by(2) {
                if 3 + i + 1 < response.len() {
                    let val = u16::from_be_bytes([response[3 + i], response[3 + i + 1]]);
                    values.push(val);
                }
            }
            Ok(values)
        }
        0x01 | 0x02 => {
            // Read bit-based response (coils or discrete inputs)
            if response.len() < 3 + byte_count {
                return Err(anyhow!("Response too short for bit data"));
            }
            let mut values = Vec::new();
            for byte_idx in 0..byte_count {
                let byte = response[3 + byte_idx];
                for bit_idx in 0..8 {
                    let coil_val = if (byte & (1 << bit_idx)) != 0 { 1 } else { 0 };
                    values.push(coil_val);
                }
            }
            Ok(values)
        }
        _ => Ok(vec![]),
    }
}

/// Read one data update from source
async fn read_one_data_update(
    source: &DataSource,
    station_id: u8,
    reg_mode: crate::protocol::status::types::modbus::RegisterMode,
    register_address: u16,
    register_length: u16,
) -> Result<Vec<u16>> {
    match source {
        DataSource::Manual => Ok(vec![]),
        DataSource::File(path) => {
            let file = std::fs::File::open(path)?;
            let mut reader = BufReader::new(file);
            let mut line = String::new();
            reader.read_line(&mut line)?;
            parse_data_line(
                &line,
                station_id,
                reg_mode,
                register_address,
                register_length,
            )
        }
        DataSource::Pipe(path) => {
            let file = std::fs::File::open(path)?;
            let mut reader = BufReader::new(file);
            let mut line = String::new();
            reader.read_line(&mut line)?;
            parse_data_line(
                &line,
                station_id,
                reg_mode,
                register_address,
                register_length,
            )
        }
        DataSource::MqttServer(url) => {
            // MQTT: connect and wait for a single publish
            // Use spawn_blocking to avoid blocking the async runtime
            let url = url.clone();
            let join_result = tokio::task::spawn_blocking(move || {
                log::debug!("Connecting to MQTT broker: {}", url);
                let parsed_url =
                    url::Url::parse(&url).map_err(|e| anyhow!("Invalid MQTT URL: {}", e))?;
                let host = parsed_url
                    .host_str()
                    .ok_or_else(|| anyhow!("MQTT URL must have a host"))?;
                let port = parsed_url.port().unwrap_or(1883);
                let topic = parsed_url.path().trim_start_matches('/');
                if topic.is_empty() {
                    return Err(anyhow!("MQTT URL must include a topic path"));
                }

                let client_id = format!("aoba_{}", uuid::Uuid::new_v4());
                let mqtt_options = rumqttc::MqttOptions::new(&client_id, host, port);
                let (client, mut connection) = rumqttc::Client::new(mqtt_options, 10);
                client
                    .subscribe(topic, rumqttc::QoS::AtMostOnce)
                    .map_err(|e| anyhow!("Failed to subscribe to MQTT topic: {}", e))?;

                for notification in connection.iter() {
                    if let Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(publish))) =
                        notification
                    {
                        let payload = String::from_utf8_lossy(&publish.payload);
                        return parse_data_line(
                            &payload,
                            station_id,
                            reg_mode,
                            register_address,
                            register_length,
                        );
                    }
                }

                Err(anyhow!("MQTT connection closed before receiving a message"))
            })
            .await
            .map_err(|e| anyhow!("MQTT task panicked: {}", e))?;

            join_result
        }
        DataSource::HttpServer(_) => {
            // HTTP server sends updates via a separate daemon; return empty initial values
            log::debug!("HTTP Server mode - returning empty initial values");
            Ok(vec![])
        }
        DataSource::IpcPipe(path) => {
            let file = std::fs::File::open(path)?;
            let mut reader = BufReader::new(file);
            let mut line = String::new();
            reader.read_line(&mut line)?;
            parse_data_line(
                &line,
                station_id,
                reg_mode,
                register_address,
                register_length,
            )
        }
    }
}

/// Run IPC socket server for master mode (synchronous, blocking)
/// Accepts connections from clients and updates the modbus storage with received data
fn run_ipc_socket_server_sync(
    socket_path: &str,
    storage: Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
    station_id: u8,
    reg_mode: crate::protocol::status::types::modbus::RegisterMode,
    register_address: u16,
    register_length: u16,
    changed_ranges: Arc<Mutex<Vec<(u16, u16, Instant)>>>,
) -> Result<()> {
    log::info!("Creating IPC socket listener at {socket_path}");

    let listener_target = resolve_ipc_socket_target(socket_path)?;
    let listener = match &listener_target {
        #[cfg(unix)]
        IpcSocketTarget::File { name, path } => {
            if path.exists() {
                log::warn!("Removing existing socket file: {path:?}");
                let _ = std::fs::remove_file(path);
            }
            ListenerOptions::new().name(name.clone()).create_sync()
        }
        IpcSocketTarget::Namespaced(name) => {
            ListenerOptions::new().name(name.clone()).create_sync()
        }
    }?;

    log::info!("IPC socket listener created, waiting for connections...");

    let connection_counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    loop {
        // Accept incoming connection (blocking - this is ok since we're in a blocking thread)
        let stream = match listener.accept() {
            Ok(stream) => stream,
            Err(e) => {
                log::error!("Failed to accept connection: {e}");
                std::thread::sleep(std::time::Duration::from_secs(1));
                continue;
            }
        };

        let conn_id = connection_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        log::info!("Accepted IPC connection #{conn_id}");

        // Clone resources for the connection handler
        let storage_clone = storage.clone();
        let changed_ranges_clone = changed_ranges.clone();

        // Build a context object to reduce function parameters
        let ipc_ctx = Arc::new(IpcConnectionContext {
            storage: storage_clone,
            station_id,
            reg_mode,
            register_address,
            register_length,
            changed_ranges: changed_ranges_clone,
        });

        // Handle connection in a separate thread (synchronous)
        std::thread::spawn(move || {
            if let Err(e) = handle_ipc_connection_sync(stream, conn_id, ipc_ctx.clone()) {
                log::error!("Connection #{conn_id} error: {e}");
            }
            log::info!("Connection #{conn_id} closed");
        });
    }
}

enum IpcSocketTarget {
    Namespaced(Name<'static>),
    #[cfg(unix)]
    File {
        name: Name<'static>,
        path: PathBuf,
    },
}

fn resolve_ipc_socket_target(socket_path: &str) -> Result<IpcSocketTarget> {
    #[cfg(unix)]
    {
        // Prefer filesystem sockets for paths containing '/' (absolute or relative paths)
        // This ensures paths like "/tmp/socket.sock" are treated as filesystem sockets
        if socket_path.contains('/') {
            let name = socket_path.to_fs_name::<GenericFilePath>()?;
            let path = PathBuf::from(socket_path);
            return Ok(IpcSocketTarget::File {
                name: name.into_owned(),
                path,
            });
        }

        // For paths without '/', try namespaced socket first
        if let Ok(ns) = socket_path.to_ns_name::<GenericNamespaced>() {
            return Ok(IpcSocketTarget::Namespaced(ns.into_owned()));
        }

        // Fallback to filesystem socket
        let name = socket_path.to_fs_name::<GenericFilePath>()?;
        let path = PathBuf::from(socket_path);
        Ok(IpcSocketTarget::File {
            name: name.into_owned(),
            path,
        })
    }

    #[cfg(not(unix))]
    {
        let ns = socket_path.to_ns_name::<GenericNamespaced>()?;
        Ok(IpcSocketTarget::Namespaced(ns.into_owned()))
    }
}

/// Context for IPC connection handler to avoid too many function arguments
struct IpcConnectionContext {
    storage: Arc<Mutex<rmodbus::server::storage::ModbusStorageSmall>>,
    station_id: u8,
    reg_mode: crate::protocol::status::types::modbus::RegisterMode,
    register_address: u16,
    register_length: u16,
    changed_ranges: Arc<Mutex<Vec<(u16, u16, Instant)>>>,
}

/// Handle a single IPC connection for master mode (synchronous)
/// Accepts JSON data (Vec<StationConfig>) and updates storage
fn handle_ipc_connection_sync(
    mut stream: interprocess::local_socket::Stream,
    conn_id: usize,
    ctx: Arc<IpcConnectionContext>,
) -> Result<()> {
    use std::io::{BufRead, BufReader, Write};

    log::info!("Connection #{conn_id}: Ready to receive JSON data");

    // Use BufReader for line-based reading
    let mut reader = BufReader::new(&mut stream);

    loop {
        // Read one line (JSON data)
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;

        if bytes_read == 0 {
            // Connection closed
            log::info!("Connection #{conn_id}: Client closed connection");
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        log::info!("Connection #{conn_id}: Received data: {line}");

        // Parse JSON data as Vec<StationConfig>
        let stations: Vec<crate::protocol::status::types::modbus::StationConfig> =
            match serde_json::from_str(line) {
                Ok(data) => data,
                Err(e) => {
                    let error_response = serde_json::json!({
                        "success": false,
                        "error": format!("Invalid JSON: {e}")
                    });
                    let response_str = serde_json::to_string(&error_response)?;
                    let stream_ref = reader.get_mut();
                    writeln!(stream_ref, "{response_str}")?;
                    stream_ref.flush()?;
                    continue;
                }
            };

        // Extract values from StationConfig
        let values = match extract_values_from_station_configs(
            &stations,
            ctx.station_id,
            ctx.reg_mode,
            ctx.register_address,
            ctx.register_length,
        ) {
            Ok(vals) => vals,
            Err(e) => {
                let error_response = serde_json::json!({
                    "success": false,
                    "error": format!("Failed to extract values: {e}")
                });
                let response_str = serde_json::to_string(&error_response)?;
                let stream_ref = reader.get_mut();
                writeln!(stream_ref, "{response_str}")?;
                stream_ref.flush()?;
                continue;
            }
        };

        log::info!(
            "Connection #{conn_id}: Extracted {} values for station_id={}, updating storage",
            values.len(),
            ctx.station_id
        );

        // Update storage with new values
        {
            let mut context = ctx.storage.lock().unwrap();
            match ctx.reg_mode {
                crate::protocol::status::types::modbus::RegisterMode::Holding => {
                    for (i, &val) in values.iter().enumerate() {
                        context.set_holding(ctx.register_address + i as u16, val)?;
                    }
                }
                crate::protocol::status::types::modbus::RegisterMode::Coils => {
                    for (i, &val) in values.iter().enumerate() {
                        context.set_coil(ctx.register_address + i as u16, val != 0)?;
                    }
                }
                crate::protocol::status::types::modbus::RegisterMode::DiscreteInputs => {
                    for (i, &val) in values.iter().enumerate() {
                        context.set_discrete(ctx.register_address + i as u16, val != 0)?;
                    }
                }
                crate::protocol::status::types::modbus::RegisterMode::Input => {
                    for (i, &val) in values.iter().enumerate() {
                        context.set_input(ctx.register_address + i as u16, val)?;
                    }
                }
            }
        }

        // Update changed ranges to bypass debounce
        {
            let mut ranges = ctx.changed_ranges.lock().unwrap();
            let end_addr = ctx.register_address + ctx.register_length;
            ranges.push((ctx.register_address, end_addr, Instant::now()));
        }

        log::info!("Connection #{conn_id}: Storage updated successfully");

        // Send success response
        let success_response = serde_json::json!({
            "success": true,
            "message": format!("Updated {} values", values.len())
        });
        let response_str = serde_json::to_string(&success_response)?;
        let stream_ref = reader.get_mut();
        writeln!(stream_ref, "{response_str}")?;
        stream_ref.flush()?;
    }

    Ok(())
}
