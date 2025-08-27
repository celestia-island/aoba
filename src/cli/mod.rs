pub mod actions;

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
        .get_matches()
}
