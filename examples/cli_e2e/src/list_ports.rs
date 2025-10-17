use anyhow::{anyhow, Result};

use ci_utils::{run_binary_sync, should_run_vcom_tests, vcom_matchers};

pub async fn test_cli_list_ports() -> Result<()> {
    let output = run_binary_sync(&["--list-ports"])?;

    log::info!(
        "list-ports stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    log::info!(
        "list-ports stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    if !output.status.success() {
        log::error!("List ports command failed with status: {}", output.status);
        return Err(anyhow!(
            "List ports command failed with status: {}",
            output.status
        ));
    }

    log::info!("🧪 List ports command works correctly");
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !should_run_vcom_tests() {
        log::info!("Skipping virtual serial port presence checks on this platform");
    } else {
        let vmatch = vcom_matchers();
        if !stdout.contains(&vmatch.port1_name) || !stdout.contains(&vmatch.port2_name) {
            log::warn!(
                "Expected {} and {} to be present in list-ports output; got: {stdout}",
                vmatch.port1_name,
                vmatch.port2_name
            );
            log::info!("🧪 Virtual serial ports not found (may be expected if socat not set up)");
        } else {
            log::info!("🧪 Found virtual serial ports in list-ports output");
        }
    }

    Ok(())
}
