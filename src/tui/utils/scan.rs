use anyhow::{anyhow, Result};

use crate::tui::utils::bus::CoreToUi;

/// Perform a ports scan and update status. Returns Ok(true) if a scan ran, Ok(false) if skipped
/// because another scan was already in progress.
///
/// NOTE: Port scanning with runtime handles disabled after PortOwner removal.
/// TUI now uses CLI subprocesses exclusively, and ports are managed differently.
/// Scan functionality needs to be refactored to work with new architecture.
///
/// For now, this function is stubbed out and just sends a refresh signal.
pub fn scan_ports(core_tx: &flume::Sender<CoreToUi>, scan_in_progress: &mut bool) -> Result<bool> {
    // Temporarily disabled - needs refactoring for new PortData structure
    // Just send refresh and return
    core_tx
        .send(CoreToUi::Refreshed)
        .map_err(|err| anyhow!("failed to send Refreshed: {err}"))?;
    *scan_in_progress = false;
    Ok(false)
}
