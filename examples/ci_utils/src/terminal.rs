use anyhow::{anyhow, Result};

use std::{
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicBool, Ordering},
};

// Global flag to track if we've already built the binary
static BINARY_BUILT: AtomicBool = AtomicBool::new(false);

/// Build the project's debug binary for a specific bin name and return the path to the executable.
/// This uses `cargo build --bin <bin_name>` to limit work to the requested binary and uses the
/// debug profile to speed up builds during testing.
///
/// Note: This function uses a global flag to ensure compilation only happens once per test run.
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

    let bin_path = workspace_root.join("target").join("debug").join(exe_name);

    // Check if we've already built the binary in this test run
    if BINARY_BUILT.load(Ordering::Relaxed) && bin_path.exists() {
        log::info!(
            "✅ Binary already built, skipping compilation: {}",
            bin_path.display()
        );
        return Ok(bin_path);
    }

    log::info!("🔧 Building debug binary for: {bin_name}");

    let status = Command::new("cargo")
        .args(["build", "--bin", bin_name])
        .current_dir(&workspace_root)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| anyhow!("Failed to execute cargo build: {e}"))?;

    if !status.success() {
        return Err(anyhow!("cargo build failed with status: {status}"));
    }

    if !bin_path.exists() {
        return Err(anyhow!("Binary not found at: {}", bin_path.display()));
    }

    // Mark that we've built the binary
    BINARY_BUILT.store(true, Ordering::Relaxed);
    log::info!(
        "✅ Binary built successfully: {path}",
        path = bin_path.display()
    );

    Ok(bin_path)
}

/// Run a binary synchronously and return its Output. `bin_path` should point to the built
/// executable (usually from `build_debug_bin`). `args` are passed to the process.
/// Build the debug binary for `aoba` if needed and run it synchronously with `args`.
/// Returns the `std::process::Output`.
pub fn run_binary_sync(args: &[&str]) -> Result<Output> {
    // Ensure aoba is built in debug mode (will be fast if already built)
    let bin_path = build_debug_bin("aoba")?;

    log::info!("▶️ Running binary: {} {:?}", bin_path.display(), args);
    let output = Command::new(&bin_path)
        .args(args)
        .output()
        .map_err(|e| anyhow!("Failed to execute binary {}: {}", bin_path.display(), e))?;

    Ok(output)
}

/// Spawn a process using `expectrl::spawn` and return a boxed `Expect` trait object.
/// This is useful for TUI tests that need to interact with the process via a pty.
pub fn spawn_expect_process(args: &[&str]) -> Result<impl expectrl::Expect> {
    // Build the debug binary for aoba if needed and spawn it with args.
    let bin_path = build_debug_bin("aoba")?;

    let mut cmd = bin_path.display().to_string();
    for a in args {
        cmd.push(' ');
        cmd.push_str(a);
    }

    log::info!("🟢 Spawning expectrl process: {cmd}");

    // If spawning TUI, prepend AOBA_LOG_FILE environment variable to command
    let cmd_with_env = if args.contains(&"--tui") {
        let tui_log = "/tmp/tui_e2e.log";
        log::info!("   TUI logs will be written to {tui_log}");
        format!("env AOBA_LOG_FILE={tui_log} {cmd}")
    } else {
        cmd
    };

    let session = expectrl::spawn(&cmd_with_env)
        .map_err(|e| anyhow!("Failed to spawn process via expectrl: {e}"))?;

    Ok(session)
}
