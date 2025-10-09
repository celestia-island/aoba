pub mod actions;
pub mod cleanup;
pub mod modbus;

use clap::{Arg, ArgMatches, Command};

/// Parse command line arguments and return ArgMatches.
pub fn parse_args() -> ArgMatches {
    Command::new("aoba")
        .arg(
            Arg::new("gui")
                .long("gui")
                .short('g')
                .help("Force GUI mode")
                .action(clap::ArgAction::SetTrue),
        )
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
        .get_matches()
}
