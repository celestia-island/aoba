use anyhow::{anyhow, Result};

use crate::utils::run_binary_sync;

pub async fn test_cli_help() -> Result<()> {
    let output = run_binary_sync(&["--help"])?;

    log::info!("help stdout: {}", String::from_utf8_lossy(&output.stdout));
    log::info!("help stderr: {}", String::from_utf8_lossy(&output.stderr));

    if !output.status.success() {
        log::error!("Help command failed with status: {}", output.status);
        return Err(anyhow!(
            "Help command failed with status: {}",
            output.status
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains("Usage: aoba") {
        log::error!("Help output doesn't contain expected usage text");
        return Err(anyhow!("Help output doesn't contain expected usage text"));
    }

    log::info!("🧪 Help command works correctly");
    Ok(())
}
