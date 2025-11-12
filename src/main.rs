use anyhow::Result;

use aoba::{
    cli::{self, actions, cleanup},
    init_common, start_tui,
};

#[tokio::main]
async fn main() -> Result<()> {
    let matches = cli::parse_args();

    if let Some(log_file) = matches.get_one::<String>("log-file") {
        std::env::set_var("AOBA_LOG_FILE", log_file);
    }

    // Console launcher: keep it simple and let the OS / terminal manage stdio.
    init_common();

    if matches.get_flag("enable-virtual-ports") {
        aoba::protocol::tty::enable_virtual_port_hint();
        log::info!("🔌 Virtual VCOM port detection enabled");
    }

    // Ensure registered cleanup handlers are run on Ctrl-C and on panic
    // Note: using ctrlc crate to register handler cross-platform
    {
        let _ = ctrlc::set_handler(|| {
            // Best-effort cleanup
            cleanup::run_cleanups();
            // Give time for cleanup to complete (port release, etc.)
            std::thread::sleep(std::time::Duration::from_millis(300));
            // After cleanup, exit
            std::process::exit(130);
        });
    }

    // Panic hook: run cleanups before unwinding/abort
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        cleanup::run_cleanups();
        default_panic(info);
    }));

    // Handle configuration mode first
    if actions::handle_config_mode(&matches) {
        return Ok(());
    }

    // One-shot actions (e.g., --list-ports). If handled, exit.
    if actions::run_one_shot_actions(&matches) {
        return Ok(());
    }

    // If TUI requested, run in this process so it inherits the terminal.
    if matches.get_flag("tui") {
        start_tui(&matches).await?;
        return Ok(());
    }

    // Default: always start TUI mode
    start_tui(&matches).await?;

    Ok(())
}
