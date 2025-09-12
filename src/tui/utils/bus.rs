use flume::{Receiver, Sender};

/// Messages sent from UI thread to core worker thread.
#[derive(Debug)]
pub enum UiToCore {
    /// Request an immediate full port / device refresh (equivalent to pressing 'r').
    Refresh,
    /// Graceful shutdown request.
    Quit,
    /// Pause polling / stop master polling loop.
    PausePolling,
    /// Resume polling / start master polling loop.
    ResumePolling,
    /// Navigate up in the current list/menu.
    NavigateUp,
    /// Navigate down in the current list/menu.
    NavigateDown,
    /// Enter the selected item/subpage.
    EnterSelection,
    /// Go back to the previous page/level.
    GoBack,
    /// Force an immediate port scan.
    ScanPorts,
    /// Start editing the currently selected field.
    StartEdit,
    /// Cancel current editing operation.
    CancelEdit,
    /// Input character during editing.
    InputChar(char),
    /// Backspace during editing.
    Backspace,
    /// Confirm/apply the current edit.
    ConfirmEdit,
}

/// Messages sent from core worker thread back to UI thread.
#[derive(Debug)]
pub enum CoreToUi {
    /// Core completed a cycle of background work; UI may redraw.
    Tick,
    /// Core applied a refresh; UI may want to redraw sooner.
    Refreshed,
    /// Core encountered an error condition (already stored in Status, provided for wake-up).
    Error,
}

/// Simple holder passed into UI loop containing the receiving side from core and the sending side to core.
#[derive(Clone)]
pub struct Bus {
    pub core_rx: Receiver<CoreToUi>,
    pub ui_tx: Sender<UiToCore>,
}

impl Bus {
    pub fn new(core_rx: Receiver<CoreToUi>, ui_tx: Sender<UiToCore>) -> Self {
        Self { core_rx, ui_tx }
    }
}
