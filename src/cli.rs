use clap::{Arg, ArgMatches, Command};

/// 解析命令行参数，返回 ArgMatches
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
        .get_matches()
}
