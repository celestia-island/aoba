use chrono::{DateTime, Local};

use crate::tui::status::{self as types, Status};

/// Set or clear a transient UI error in the provided `Status` snapshot (mutable).
/// Accepts an optional `(message, timestamp)` tuple and converts it to the
/// generated `ErrorInfo` struct under `types`.
pub fn ui_error_set(status: &mut Status, error: Option<(String, DateTime<Local>)>) {
    if let Some((message, timestamp)) = error {
        status.temporarily.error = Some(types::ErrorInfo { message, timestamp });
    } else {
        status.temporarily.error = None;
    }
}
