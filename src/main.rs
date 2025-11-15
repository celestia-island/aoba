use anyhow::Result;

use aoba::{
    cli::{self, actions, cleanup},
    init_common,
    protocol::tty::enable_virtual_port_hint,
    start_tui,
};

#[tokio::main]
async fn main() -> Result<()> {
    let matches = cli::parse_args();

    if let Some(log_file) = matches.get_one::<String>("log-file") {
        std::env::set_var("AOBA_LOG_FILE", log_file);
    }

    // For daemon mode, skip init_common and initialize dual logger later
    // For all other modes, use normal initialization
    if !matches.get_flag("daemon") {
        // Console launcher: keep it simple and let the OS / terminal manage stdio.
        init_common();
    }

    if matches.get_flag("enable-virtual-ports") {
        enable_virtual_port_hint();
        log::info!("🔌 Virtual VCOM port detection enabled");
    }

    // Ensure registered cleanup handlers are run on Ctrl-C and on panic
    // Note: using ctrlc crate to register handler cross-platform
    {
        let _ = ctrlc::set_handler(|| {
            // Best-effort cleanup
            cleanup::run_cleanups();
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
    if actions::handle_config_mode(&matches).await {
        return Ok(());
    }

    // One-shot actions (e.g., --list-ports). If handled, exit.
    // Note: Some one-shot actions (like MQTT-based master-provide) need to run in a blocking context
    // because they use synchronous MQTT clients that create their own tokio runtime.
    // We use spawn_blocking to avoid "runtime within runtime" panics.
    let matches_clone = matches.clone();
    if actions::run_one_shot_actions(&matches_clone).await {
        std::process::exit(0);
    }

    // Handle daemon mode
    if matches.get_flag("daemon") {
        // Initialize i18n (normally done in init_common)
        aoba::utils::i18n::init_i18n();
        
        // Initialize dual logger for daemon mode (outputs to both file and terminal)
        let log_file = if let Some(log_file) = std::env::var("AOBA_LOG_FILE").ok() {
            log_file
        } else {
            // If no log file specified, set a default one for daemon mode
            let file = format!(
                "./aoba_daemon_{}.log",
                chrono::Local::now().format("%Y%m%d_%H%M%S")
            );
            std::env::set_var("AOBA_LOG_FILE", &file);
            file
        };
        
        if let Err(err) = aoba::init_daemon_logger(&log_file) {
            eprintln!("⚠️ Failed to initialize daemon logger: {err}");
            eprintln!("⚠️ Continuing without logging...");
        }
        
        aoba::start_daemon(&matches).await?;
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
