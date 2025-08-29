pub mod entry;
pub mod pull;
pub mod slave;

use crossterm::event::KeyEvent;
use ratatui::prelude::*;

use crate::protocol::status::Status;

/// Route a KeyEvent to the active subpage.
/// Returns true if the subpage consumed the event and no further handling should occur.
pub fn handle_key_in_subpage(key: KeyEvent, app: &mut Status) -> bool {
    use crossterm::event::KeyCode as KC;

    // Always let 'q' bubble up to the top-level quit handler (don't consume it here).
    if let KC::Char('q') | KC::Char('Q') = key.code {
        return false;
    }

    if let Some(sub) = app.active_subpage {
        match sub {
            crate::protocol::status::RightMode::Master => {
                return slave::handle_subpage_key(key, app)
            }
            crate::protocol::status::RightMode::SlaveStack => {
                return pull::handle_subpage_key(key, app)
            }
            crate::protocol::status::RightMode::Listen => {
                return entry::handle_subpage_key(key, app)
            }
        }
    }
    false
}

pub fn render_panels(f: &mut Frame, area: Rect, app: &Status) {
    // If a subpage is active, render it full-screen; otherwise render the normal entry view
    if let Some(sub) = app.active_subpage {
        match sub {
            crate::protocol::status::RightMode::Master => slave::render_slave(f, area, app),
            crate::protocol::status::RightMode::SlaveStack => pull::render_pull(f, area, app),
            crate::protocol::status::RightMode::Listen => {
                // reuse entry's listen rendering but full area
                entry::render_entry(f, area, app)
            }
        }
    } else {
        entry::render_entry(f, area, app);
    }
}
