use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Console launcher: keep it simple and let the OS / terminal manage stdio.
    aoba::init_common();

    // Ensure registered cleanup handlers are run on Ctrl-C and on panic
    // Note: using ctrlc crate to register handler cross-platform
    {
        let _ = ctrlc::set_handler(|| {
            // Best-effort cleanup
            aoba::cli::cleanup::run_cleanups();
            // Give time for cleanup to complete (port release, etc.)
            std::thread::sleep(std::time::Duration::from_millis(300));
            // After cleanup, exit
            std::process::exit(130);
        });
    }

    // Panic hook: run cleanups before unwinding/abort
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        aoba::cli::cleanup::run_cleanups();
        default_panic(info);
    }));

    let matches = aoba::cli::parse_args();

    // Handle configuration mode first
    if aoba::cli::actions::handle_config_mode(&matches) {
        return Ok(());
    }

    // One-shot actions (e.g., --list-ports). If handled, exit.
    if aoba::cli::actions::run_one_shot_actions(&matches) {
        return Ok(());
    }

    // If TUI requested, run in this process so it inherits the terminal.
    if matches.get_flag("tui") {
        aoba::start_tui()?;
        return Ok(());
    }

    // Default: always start TUI mode
    aoba::start_tui()?;

    Ok(())
}
