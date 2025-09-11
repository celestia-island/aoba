use crate::protocol::status::Status;

// NOTE: these accessors are the initial compatibility layer. They currently
// read/write the existing `Status.ui` fields to remain non-breaking. In the
// next iteration we'll change these to prefer `AppPage::ephemeral` values.

pub fn ui_log_selected_get(st: &Status) -> usize {
    st.ui.log_selected
}

pub fn ui_log_selected_set(st: &mut Status, v: usize) {
    st.ui.log_selected = v;
}

pub fn ui_log_view_offset_get(st: &Status) -> usize {
    st.ui.log_view_offset
}

pub fn ui_log_view_offset_set(st: &mut Status, v: usize) {
    st.ui.log_view_offset = v;
}

pub fn ui_log_auto_scroll_get(st: &Status) -> bool {
    st.ui.log_auto_scroll
}

pub fn ui_log_auto_scroll_set(st: &mut Status, v: bool) {
    st.ui.log_auto_scroll = v;
}

// Return a cloned view of the logs for read-only rendering paths. Cloning avoids
// holding long-lived borrows on Status while rendering complex widgets.
pub fn ui_logs_get(st: &Status) -> Vec<crate::protocol::status::LogEntry> {
    st.ui.logs.clone()
}

// --- Additional compatibility accessors (initially forwarding to Status.ui)

pub fn ui_selected_get(st: &Status) -> usize {
    st.ui.selected
}

pub fn ui_selected_set(st: &mut Status, v: usize) {
    st.ui.selected = v;
}

pub fn ui_input_buffer_get(st: &Status) -> String {
    st.ui.input_buffer.clone()
}

pub fn ui_input_buffer_set(st: &mut Status, v: String) {
    st.ui.input_buffer = v;
}

pub fn ui_input_editing_get(st: &Status) -> bool {
    st.ui.input_editing
}

pub fn ui_input_editing_set(st: &mut Status, v: bool) {
    st.ui.input_editing = v;
}

pub fn ui_input_mode_get(st: &Status) -> crate::protocol::status::InputMode {
    st.ui.input_mode
}

pub fn ui_input_mode_set(st: &mut Status, v: crate::protocol::status::InputMode) {
    st.ui.input_mode = v;
}

// cursor is defined inside types_form / pages; provide an accessor that
// mirrors the typical usage: operate on the last page if present as a fallback.
// For now we forward to the global ui pages[0]/last cursor where applicable.
pub fn ui_cursor_get(st: &Status) -> usize {
    // Prefer per-page cursor if pages exist; fallback to 0.
    if let Some(last) = st.ui.pages.last() {
        match last {
            crate::protocol::status::Page::Entry {
                selected: _,
                input_mode: _,
                input_editing: _,
                input_buffer: _,
                app_mode: _,
            } => {
                // Entry page does not expose cursor in its struct; default 0
                0
            }
            crate::protocol::status::Page::Modbus {
                selected: _,
                subpage_active: _,
                subpage_form: _,
                subpage_tab_index: _,
                logs: _,
                log_selected: _,
                log_view_offset: _,
                log_auto_scroll: _,
                log_clear_pending: _,
                input_mode: _,
                input_editing: _,
                input_buffer: _,
                app_mode: _,
            } => {
                // Modbus page also doesn't directly have `cursor` in the Page enum; default 0
                0
            }
        }
    } else {
        0
    }
}

pub fn ui_cursor_set(_st: &mut Status, _v: usize) {
    // No-op for now: cursor lives in page-local form types (see types_form.rs).
    // Migration will route this to AppPage::ephemeral.cursor.
}

pub fn ui_master_input_buffer_get(st: &Status) -> String {
    // master_input_buffer currently lives in form types; try to get from last page snapshot
    // Fallback to empty string to preserve behavior but avoid panics.
    // Many call sites expect to borrow; returning cloned String is simplest for compatibility.
    // TODO: Prefer AppPage::ephemeral.master_input_buffer when available.
    if let Some(last) = st.ui.pages.last() {
        match last {
            crate::protocol::status::Page::Entry { .. } => String::new(),
            crate::protocol::status::Page::Modbus { .. } => String::new(),
        }
    } else {
        String::new()
    }
}

pub fn ui_master_input_buffer_set(_st: &mut Status, _v: String) {
    // No-op: page-local master_input_buffer should be updated via ephemeral structures.
}

// Return the last page snapshot (cloned) when callers need a lightweight read-only
// view of the current page. This mirrors common call sites that previously used
// `st.ui.pages.last().cloned()` directly.
pub fn ui_pages_last_get(st: &Status) -> Option<crate::protocol::status::Page> {
    st.ui.pages.last().cloned()
}

// --- Additional getters for other UI fields used in rendering (read-only compatibility)
pub fn ui_subpage_form_get(st: &Status) -> Option<crate::protocol::status::SubpageForm> {
    st.ui.subpage_form.clone()
}

pub fn ui_subpage_active_get(st: &Status) -> bool {
    st.ui.subpage_active
}

pub fn ui_log_clear_pending_get(st: &Status) -> bool {
    st.ui.log_clear_pending
}

pub fn ui_app_mode_get(st: &Status) -> crate::protocol::status::AppMode {
    st.ui.app_mode
}

pub fn ui_subpage_tab_index_get(st: &Status) -> crate::protocol::status::SubpageTab {
    st.ui.subpage_tab_index
}

pub fn ui_error_get(st: &Status) -> Option<(String, chrono::DateTime<chrono::Local>)> {
    st.ui.error.clone()
}

pub fn ui_mode_overlay_active_get(st: &Status) -> bool {
    st.ui.mode_overlay_active
}

pub fn ui_mode_overlay_index_get(st: &Status) -> crate::protocol::status::ModeOverlayIndex {
    st.ui.mode_overlay_index
}

// --- Setter accessors for write migration. These forward to Status.ui for now.
pub fn ui_subpage_form_set(st: &mut Status, v: Option<crate::protocol::status::SubpageForm>) {
    st.ui.subpage_form = v;
}

pub fn ui_subpage_active_set(st: &mut Status, v: bool) {
    st.ui.subpage_active = v;
}

pub fn ui_subpage_tab_index_set(st: &mut Status, v: crate::protocol::status::SubpageTab) {
    st.ui.subpage_tab_index = v;
}

pub fn ui_app_mode_set(st: &mut Status, v: crate::protocol::status::AppMode) {
    st.ui.app_mode = v;
}

pub fn ui_logs_set(st: &mut Status, v: Vec<crate::protocol::status::LogEntry>) {
    st.ui.logs = v;
}

// Per-port helpers: operate on PerPortState logs by port name. These are
// convenience accessors to centralize per-port mutations and keep callsites
// consistent with the ui accessors used for the global page.
pub fn per_port_logs_get(st: &Status, port_name: &str) -> Vec<crate::protocol::status::LogEntry> {
    st.per_port
        .states
        .get(port_name)
        .map(|ps| ps.logs.clone())
        .unwrap_or_default()
}

pub fn per_port_logs_set(
    st: &mut Status,
    port_name: &str,
    v: Vec<crate::protocol::status::LogEntry>,
) {
    if let Some(ps) = st.per_port.states.get_mut(port_name) {
        ps.logs = v;
    } else {
        // If no per-port state exists, create one with sensible defaults.
        let snap = crate::protocol::status::status_common::PerPortState {
            subpage_active: false,
            subpage_form: None,
            subpage_tab_index: crate::protocol::status::SubpageTab::Config,
            logs: v,
            log_selected: 0,
            log_view_offset: 0,
            log_auto_scroll: true,
            log_clear_pending: false,
            input_mode: crate::protocol::status::InputMode::Ascii,
            input_editing: false,
            input_buffer: String::new(),
            app_mode: crate::protocol::status::AppMode::Modbus,
            page: None,
        };
        st.per_port.states.insert(port_name.to_string(), snap);
    }
}

pub fn ui_log_clear_pending_set(st: &mut Status, v: bool) {
    st.ui.log_clear_pending = v;
}

pub fn ui_error_set(st: &mut Status, v: Option<(String, chrono::DateTime<chrono::Local>)>) {
    st.ui.error = v;
}

pub fn ui_mode_overlay_active_set(st: &mut Status, v: bool) {
    st.ui.mode_overlay_active = v;
}

pub fn ui_mode_overlay_index_set(st: &mut Status, v: crate::protocol::status::ModeOverlayIndex) {
    st.ui.mode_overlay_index = v;
}
