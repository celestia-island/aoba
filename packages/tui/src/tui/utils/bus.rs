use std::sync::atomic::{AtomicBool, Ordering};

use flume::{Receiver, Sender};

static REFRESH_PENDING: AtomicBool = AtomicBool::new(false);

/// Messages sent from UI thread to core worker thread.
#[derive(Debug, Clone, PartialEq)]
pub enum UiToCore {
    /// Request an immediate UI redraw without forcing a port rescan.
    Refresh,
    /// Trigger a full port rescan (equivalent to pressing the Refresh entry).
    RescanPorts,
    /// Graceful shutdown request.
    Quit,
    /// Pause polling / stop master polling loop.
    PausePolling,
    /// Resume polling / start master polling loop.
    ResumePolling,
    /// Toggle (start/stop) per-port runtime. Contains port name.
    ToggleRuntime(String),
    /// Restart per-port runtime (stop if running, then start). Contains port name.
    RestartRuntime(String),
    /// Send register update to CLI subprocess via IPC
    SendRegisterUpdate {
        port_name: String,
        station_id: u8,
        register_type: String,
        start_address: u16,
        values: Vec<u16>,
    },
}

/// Messages sent from core worker thread back to UI thread.
#[derive(Debug, Clone, PartialEq)]
pub enum CoreToUi {
    /// Core completed a cycle of background work; UI may redraw.
    Tick,
    /// Core applied a refresh; UI may want to redraw sooner.
    Refreshed,
    /// Core encountered an error condition (already stored in Status, provided for wake-up).
    Error,
    /// Core is requesting UI to quit.
    Quit,
}

/// Simple holder passed into UI loop containing the receiving side from core and the sending side to core.
#[derive(Debug, Clone)]
pub struct Bus {
    pub core_rx: Receiver<CoreToUi>,
    pub ui_tx: Sender<UiToCore>,
}

impl Bus {
    pub fn new(core_rx: Receiver<CoreToUi>, ui_tx: Sender<UiToCore>) -> Self {
        Self { core_rx, ui_tx }
    }
}

/// Try to enqueue a refresh message unless one is already pending.
/// Returns `Ok(true)` when a message was sent, `Ok(false)` when it was coalesced.
pub fn request_refresh(sender: &Sender<UiToCore>) -> Result<bool, flume::SendError<UiToCore>> {
    // Only one Refresh should be outstanding to avoid starving the writer thread.
    if REFRESH_PENDING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Ok(false);
    }

    sender.send(UiToCore::Refresh).map(|_| true)
}

/// Mark the refresh flag as cleared so the next request can be queued.
pub fn mark_refresh_complete() {
    REFRESH_PENDING.store(false, Ordering::Release);
}
