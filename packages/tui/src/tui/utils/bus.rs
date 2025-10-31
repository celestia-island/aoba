use flume::{Receiver, Sender};

/// Messages sent from UI thread to core worker thread.
#[derive(Debug, Clone, PartialEq)]
pub enum UiToCore {
    /// Request an immediate full port / device refresh (equivalent to pressing 'r').
    Refresh,
    /// Graceful shutdown request.
    Quit,
    /// Pause polling / stop master polling loop.
    PausePolling,
    /// Resume polling / start master polling loop.
    ResumePolling,
    /// Toggle (start/stop) per-port runtime. Contains port name.
    ToggleRuntime(String),
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
