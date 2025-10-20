use chrono::{DateTime, Local};

use crate::tui::global_status::{self as types, Status};

/// Set or clear a transient UI error in the provided `Status` snapshot (mutable).
/// Accepts an optional `(message, timestamp)` tuple and converts it to the
/// generated `ErrorInfo` struct under `types`.
pub fn ui_error_set(_status: &mut Status, _error: Option<(String, DateTime<Local>)>) {
    if let Some((message, timestamp)) = _error {
        _status.temporarily.error = Some(types::ErrorInfo { message, timestamp });
    } else {
        _status.temporarily.error = None;
    }
}
