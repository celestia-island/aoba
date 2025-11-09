pub mod actions;
pub mod cleanup;
pub mod config;
pub mod modbus;
pub mod status;

use clap::{Arg, ArgMatches, Command};

/// Parse command line arguments and return ArgMatches.
pub fn parse_args() -> ArgMatches {
    Command::new("aoba")
        .arg(
            Arg::new("tui")
                .long("tui")
                .short('t')
                .help("Force TUI mode")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("list-ports")
                .long("list-ports")
                .short('l')
                .help("List all available serial ports and exit")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("json")
                .long("json")
                .short('j')
                .help("Output one-shot results in JSON format")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("log-file")
                .long("log-file")
                .help("Write detailed logs to the specified file (overrides AOBA_LOG_FILE)")
                .value_name("FILE"),
        )
        .arg(
            Arg::new("config")
                .long("config")
                .short('c')
                .help("Load configuration from JSON file")
                .value_name("FILE")
                .conflicts_with_all(["slave-listen", "slave-listen-persist", "slave-poll", "slave-poll-persist", "master-provide", "master-provide-persist"]),
        )
        .arg(
            Arg::new("config-json")
                .long("config-json")
                .help("Load configuration from JSON string")
                .value_name("JSON")
                .conflicts_with_all(["slave-listen", "slave-listen-persist", "slave-poll", "slave-poll-persist", "master-provide", "master-provide-persist", "config"]),
        )
        .arg(
            Arg::new("slave-listen")
                .long("slave-listen")
                .help("Modbus slave: listen for requests and respond once, then exit")
                .value_name("PORT")
                .conflicts_with_all(["slave-listen-persist", "slave-poll", "master-provide", "master-provide-persist"]),
        )
        .arg(
            Arg::new("slave-listen-persist")
                .long("slave-listen-persist")
                .help("Modbus slave: continuously listen for requests and respond (JSONL output)")
                .value_name("PORT")
                .conflicts_with_all(["slave-listen", "slave-poll", "master-provide", "master-provide-persist"]),
        )
        .arg(
            Arg::new("slave-poll")
                .long("slave-poll")
                .help("Modbus slave: send request and wait for response once, then exit (acts as client)")
                .value_name("PORT")
                .conflicts_with_all(["slave-listen", "slave-listen-persist", "slave-poll-persist", "master-provide", "master-provide-persist"]),
        )
        .arg(
            Arg::new("slave-poll-persist")
                .long("slave-poll-persist")
                .help("Modbus slave: continuously poll for data and output responses (JSONL output)")
                .value_name("PORT")
                .conflicts_with_all(["slave-listen", "slave-listen-persist", "slave-poll", "master-provide", "master-provide-persist"]),
        )
        .arg(
            Arg::new("master-provide")
                .long("master-provide")
                .help("Modbus master: provide data once and respond to requests, then exit")
                .value_name("PORT")
                .conflicts_with_all(["master-provide-persist", "slave-listen", "slave-listen-persist", "slave-poll"]),
        )
        .arg(
            Arg::new("master-provide-persist")
                .long("master-provide-persist")
                .help("Modbus master: continuously provide data and respond to requests (JSONL output)")
                .value_name("PORT")
                .conflicts_with_all(["master-provide", "slave-listen", "slave-listen-persist", "slave-poll"]),
        )
        .arg(
            Arg::new("station-id")
                .long("station-id")
                .help("Modbus station ID (slave address)")
                .value_name("ID")
                .default_value("1")
                .value_parser(clap::value_parser!(u8)),
        )
        .arg(
            Arg::new("register-address")
                .long("register-address")
                .help("Starting register address")
                .value_name("ADDR")
                .default_value("0")
                .value_parser(clap::value_parser!(u16)),
        )
        .arg(
            Arg::new("register-length")
                .long("register-length")
                .help("Number of registers")
                .value_name("LEN")
                .default_value("10")
                .value_parser(clap::value_parser!(u16)),
        )
        .arg(
            Arg::new("register-mode")
                .long("register-mode")
                .help("Register type: holding, input, coils, discrete")
                .value_name("MODE")
                .default_value("holding"),
        )
        .arg(
            Arg::new("data-source")
                .long("data-source")
                .help("Data source for master mode: file:<path> or pipe:<name>")
                .value_name("SOURCE")
                .requires_ifs([
                    ("master-provide", "master-provide"),
                    ("master-provide-persist", "master-provide-persist"),
                ]),
        )
        .arg(
            Arg::new("output")
                .long("output")
                .help("Output destination for slave mode: file:<path> or pipe:<name> (default: stdout)")
                .value_name("OUTPUT"),
        )
        .arg(
            Arg::new("baud-rate")
                .long("baud-rate")
                .help("Serial port baud rate")
                .value_name("BAUD")
                .default_value("9600")
                .value_parser(clap::value_parser!(u32)),
        )
        .arg(
            Arg::new("request-interval-ms")
                .long("request-interval-ms")
                .value_name("MS")
                .help("Request interval time in milliseconds for successful polls (default: 1000)")
                .value_parser(clap::value_parser!(u32))
                .default_value("1000"),
        )
        .arg(
            Arg::new("timeout-ms")
                .long("timeout-ms")
                .value_name("MS")
                .help("Timeout waiting time in milliseconds for failed requests (default: 3000)")
                .value_parser(clap::value_parser!(u32))
                .default_value("3000"),
        )
        .arg(
            Arg::new("debounce-seconds")
                .long("debounce-seconds")
                .help("Debounce window for duplicate JSON output in seconds. Default 1 second. Set to 0 to disable.")
                .value_name("SECONDS")
                .default_value("1.0")
                .value_parser(clap::value_parser!(f32)),
        )
        .arg(
            Arg::new("ipc-channel")
                .long("ipc-channel")
                .help("IPC channel UUID for TUI communication (internal use - not recommended for manual invocation)\n\
                       This parameter is used by TUI mode to establish an IPC connection with CLI subprocesses.\n\
                       Manual use is discouraged as it requires a running IPC server.")
                .value_name("UUID")
                .hide(false), // Show in help but with warning
        )
        .arg(
            Arg::new("debug-ci-e2e-test")
                .long("debug-ci-e2e-test")
                .help("Enable CI E2E test mode: periodically dump global status to /tmp/ci_cli_{port}_status.json")
                .action(clap::ArgAction::SetTrue)
                .hide(true), // Hidden from normal help output
        )
        .arg(
            Arg::new("debug-ci")
                .long("debug-ci")
                .help("Enable CI mode for IPC-based E2E testing: TUI listens for keyboard events via IPC")
                .value_name("CHANNEL_ID")
                .hide(true), // Hidden from normal help output
        )
        .arg(
            Arg::new("debug-screen-capture")
                .long("debug-screen-capture")
                .help("Enable screen capture mode: render UI once and exit immediately")
                .action(clap::ArgAction::SetTrue)
                .hide(true), // Hidden from normal help output
        )
        .arg(
            Arg::new("no-config-cache")
                .long("no-config-cache")
                .help("Disable configuration cache (do not load/save aoba_tui_config.json). Useful for E2E tests.")
                .action(clap::ArgAction::SetTrue)
                .hide(false), // Visible in help
        )
        .get_matches()
}
