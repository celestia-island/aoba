use clap::ArgMatches;

/// Whether user explicitly requested GUI mode
pub fn want_gui(matches: &ArgMatches) -> bool {
    matches.get_flag("gui")
}

/// Whether user explicitly requested TUI mode
pub fn want_tui(matches: &ArgMatches) -> bool {
    matches.get_flag("tui")
}
