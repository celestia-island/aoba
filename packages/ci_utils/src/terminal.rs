use anyhow::{anyhow, Result};
use std::{
    path::PathBuf,
    process::{Command, Output},
};

/// Locate the project's debug binary for a specific bin name and return the path to the executable.
/// Callers must ensure `cargo build --bin <bin_name>` has already been executed prior to invoking
/// this helper so that E2E workflows never hide an implicit rebuild.
/// This function tries both release and debug builds, preferring release if available.
pub fn build_debug_bin(bin_name: &str) -> Result<PathBuf> {
    // Try to find the workspace root by looking for Cargo.toml with [workspace]
    let workspace_root = std::env::current_dir()?
        .ancestors()
        .find(|p| {
            let cargo_toml = p.join("Cargo.toml");
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                content.contains("[workspace]") || content.contains("name = \"aoba\"")
            } else {
                false
            }
        })
        .map(|p| p.to_path_buf())
        .ok_or_else(|| anyhow!("Could not find workspace root"))?;

    log::info!("🔍 Workspace root: {path}", path = workspace_root.display());

    let exe_name = if cfg!(windows) {
        format!("{bin_name}.exe")
    } else {
        bin_name.to_string()
    };

    // Try release first, then debug
    let bin_paths = [
        workspace_root
            .join("target")
            .join("release")
            .join(&exe_name),
        workspace_root.join("target").join("debug").join(&exe_name),
    ];

    let bin_path = bin_paths.iter()
        .find(|p| p.exists())
        .ok_or_else(|| anyhow!(
            "Binary not found at any of: {}. Run `cargo build --bin {}` or `cargo build --release --bin {}` before triggering E2E tests.",
            bin_paths.iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", "),
            bin_name,
            bin_name
        ))?
        .to_path_buf(); // Convert &PathBuf to PathBuf

    log::info!("✅ Using prebuilt binary: {}", bin_path.display());
    Ok(bin_path)
}

/// Run a binary synchronously and return its Output. `bin_path` should point to the built
/// executable (usually from `build_debug_bin`). `args` are passed to the process.
/// Build the debug binary for `aoba` if needed and run it synchronously with `args`.
/// Returns the `std::process::Output`.
pub fn run_binary_sync(args: &[&str]) -> Result<Output> {
    // Ensure aoba has already been built in debug mode (caller must pre-run cargo build)
    let bin_path = build_debug_bin("aoba")?;

    log::info!("▶️ Running binary: {} {:?}", bin_path.display(), args);
    let output = Command::new(&bin_path)
        .args(args)
        .output()
        .map_err(|err| anyhow!("Failed to execute binary {}: {}", bin_path.display(), err))?;

    Ok(output)
}

/// Spawn a process using `expectrl::spawn` and return a boxed `Expect` trait object.
/// This is useful for TUI tests that need to interact with the process via a pty.
///
/// # Arguments
/// * `args` - Command line arguments to pass to the process
/// * `pty_size` - Optional PTY size (rows, cols). If None, uses default size.
pub fn spawn_expect_process_with_size(
    args: &[&str],
    pty_size: Option<(u16, u16)>,
) -> Result<impl expectrl::Expect> {
    // Build the debug binary for aoba if needed and spawn it with args.
    let bin_path = build_debug_bin("aoba")?;

    log::info!(
        "🟢 Spawning expectrl process: {} {}",
        bin_path.display(),
        args.join(" ")
    );

    if let Some((rows, cols)) = pty_size {
        log::info!("   PTY size: {rows}x{cols} (rows x cols)");
    }

    // If spawning TUI, set AOBA_LOG_FILE environment variable
    let tui_log_path = if args.contains(&"--tui") {
        #[cfg(windows)]
        let log_path = std::env::temp_dir().join("tui_e2e.log");
        #[cfg(not(windows))]
        let log_path = std::path::PathBuf::from("/tmp/tui_e2e.log");

        log::info!("   TUI logs will be written to {}", log_path.display());
        Some(log_path)
    } else {
        None
    };

    // Spawn using WrapperProcess which allows setting environment variables
    let mut cmd_args = vec![bin_path.to_str().unwrap().to_string()];
    cmd_args.extend(args.iter().map(|s| s.to_string()));

    let mut cmd = std::process::Command::new(&cmd_args[0]);
    cmd.args(&cmd_args[1..]);

    // Set environment variable if needed
    if let Some(log_path) = tui_log_path {
        cmd.env("AOBA_LOG_FILE", log_path.to_str().unwrap());
    }

    // Force deterministic locale so text assertions remain stable across CI machines
    cmd.env("LANGUAGE", "en_US:en");
    cmd.env("LC_ALL", "en_US.UTF-8");
    cmd.env("LANG", "en_US.UTF-8");

    // Use expectrl's spawn with Command
    #[cfg(not(windows))]
    let mut session = expectrl::session::Session::spawn(cmd)
        .map_err(|err| anyhow!("Failed to spawn process via expectrl: {err}"))?;

    #[cfg(windows)]
    let session = expectrl::session::Session::spawn(cmd)
        .map_err(|err| anyhow!("Failed to spawn process via expectrl: {err}"))?;

    // Set PTY size if specified (only available on Unix-like systems)
    #[cfg(not(windows))]
    if let Some((rows, cols)) = pty_size {
        session
            .get_process_mut()
            .set_window_size(cols, rows)
            .map_err(|err| anyhow!("Failed to set PTY window size: {err}"))?;
    }

    #[cfg(windows)]
    if pty_size.is_some() {
        log::warn!("PTY size setting is not supported on Windows platform");
    }

    Ok(session)
}

/// Spawn a process using `expectrl::spawn` with default PTY size.
/// For compatibility with existing code.
pub fn spawn_expect_process(args: &[&str]) -> Result<impl expectrl::Expect> {
    spawn_expect_process_with_size(args, None)
}

/// Spawn a process and return a concrete Session type that implements ExpectSession.
/// This is needed for terminal capture operations that require the ExpectSession trait.
///
/// # Arguments
/// * `args` - Command line arguments to pass to the process
/// * `pty_size` - Optional PTY size (rows, cols). If None, uses default size.
#[cfg(not(windows))]
pub fn spawn_expect_session_with_size(
    args: &[&str],
    pty_size: Option<(u16, u16)>,
) -> Result<
    expectrl::session::Session<
        expectrl::process::unix::UnixProcess,
        expectrl::process::unix::PtyStream,
    >,
> {
    use expectrl::session::Session;

    // Build the debug binary for aoba if needed and spawn it with args.
    let bin_path = build_debug_bin("aoba")?;

    log::info!(
        "🟢 Spawning expectrl session: {} {}",
        bin_path.display(),
        args.join(" ")
    );

    if let Some((rows, cols)) = pty_size {
        log::info!("   PTY size: {rows}x{cols} (rows x cols)");
    }

    // If spawning TUI, set AOBA_LOG_FILE environment variable
    let tui_log_path = if args.contains(&"--tui") {
        let log_path = std::path::PathBuf::from("/tmp/tui_e2e.log");
        log::info!("   TUI logs will be written to {}", log_path.display());
        Some(log_path)
    } else {
        None
    };

    // Spawn using WrapperProcess which allows setting environment variables
    let mut cmd_args = vec![bin_path.to_str().unwrap().to_string()];
    cmd_args.extend(args.iter().map(|s| s.to_string()));

    let mut cmd = std::process::Command::new(&cmd_args[0]);
    cmd.args(&cmd_args[1..]);

    // Set environment variable if needed
    if let Some(log_path) = tui_log_path {
        cmd.env("AOBA_LOG_FILE", log_path.to_str().unwrap());
    }

    // Force deterministic locale so text assertions remain stable across CI machines
    cmd.env("LANGUAGE", "en_US:en");
    cmd.env("LC_ALL", "en_US.UTF-8");
    cmd.env("LANG", "en_US.UTF-8");

    // Use expectrl's spawn with Command
    let mut session = Session::spawn(cmd)
        .map_err(|err| anyhow!("Failed to spawn process via expectrl: {err}"))?;

    // Set PTY size if specified
    if let Some((rows, cols)) = pty_size {
        session
            .get_process_mut()
            .set_window_size(cols, rows)
            .map_err(|err| anyhow!("Failed to set PTY window size: {err}"))?;
    }

    Ok(session)
}

/// Spawn a process and return a concrete Session type that implements ExpectSession.
/// Windows version (PTY sizing not supported).
#[cfg(windows)]
pub fn spawn_expect_session_with_size(
    args: &[&str],
    pty_size: Option<(u16, u16)>,
) -> Result<
    expectrl::session::Session<
        expectrl::process::windows::WinProcess,
        expectrl::process::windows::ProcessStream,
    >,
> {
    use expectrl::session::Session;

    // Build the debug binary for aoba if needed and spawn it with args.
    let bin_path = build_debug_bin("aoba")?;

    log::info!(
        "🟢 Spawning expectrl session: {} {}",
        bin_path.display(),
        args.join(" ")
    );

    if pty_size.is_some() {
        log::warn!("PTY size setting is not supported on Windows platform");
    }

    // If spawning TUI, set AOBA_LOG_FILE environment variable
    let tui_log_path = if args.contains(&"--tui") {
        let log_path = std::env::temp_dir().join("tui_e2e.log");
        log::info!("   TUI logs will be written to {}", log_path.display());
        Some(log_path)
    } else {
        None
    };

    // Spawn using WrapperProcess which allows setting environment variables
    let mut cmd_args = vec![bin_path.to_str().unwrap().to_string()];
    cmd_args.extend(args.iter().map(|s| s.to_string()));

    let mut cmd = std::process::Command::new(&cmd_args[0]);
    cmd.args(&cmd_args[1..]);

    // Set environment variable if needed
    if let Some(log_path) = tui_log_path {
        cmd.env("AOBA_LOG_FILE", log_path.to_str().unwrap());
    }

    // Force deterministic locale so text assertions remain stable across CI machines
    cmd.env("LANGUAGE", "en_US:en");
    cmd.env("LC_ALL", "en_US.UTF-8");
    cmd.env("LANG", "en_US.UTF-8");

    // Use expectrl's spawn with Command
    let session = Session::spawn(cmd)
        .map_err(|err| anyhow!("Failed to spawn process via expectrl: {err}"))?;

    Ok(session)
}

/// Spawn a process and return a concrete Session type with default PTY size.
pub fn spawn_expect_session(args: &[&str]) -> Result<impl crate::snapshot::ExpectSession> {
    spawn_expect_session_with_size(args, None)
}
