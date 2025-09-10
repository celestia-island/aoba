use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::protocol::status::Status;

/// Experimental read-only accessor for `Status`.
///
/// - `s` is the shared `Arc<Mutex<Status>>` used across the app.
/// - `f` is a user-provided closure that receives a reference to `Status` and
///   returns `Result<R, E>` (mapped to anyhow::Result here). The closure may
///   borrow from `Status`. The returned value will be cloned before leaving
///   the function to avoid lifetime issues. Therefore `R: Clone` is required.
pub fn read_status<R, F>(s: &Arc<Mutex<Status>>, f: F) -> Result<R>
where
    F: FnOnce(&Status) -> Result<R>,
    R: Clone,
{
    let guard = s
        .lock()
        .map_err(|e| anyhow::anyhow!("status lock poisoned: {}", e))?;
    // Call user closure with borrowed reference
    let val = f(&*guard)?;
    // Clone once to decouple lifetime
    Ok(val.clone())
}

/// Experimental write accessor for `Status`.
///
/// - `f` is a FnMut that receives a mutable reference and may mutate status.
/// - The closure returns a `Result<R>`; the returned value will be cloned
///   before returning to avoid lifetime issues. Use `Ok(())` if no value is
///   needed.
pub fn write_status<R, F>(s: &Arc<Mutex<Status>>, mut f: F) -> Result<R>
where
    F: FnMut(&mut Status) -> Result<R>,
    R: Clone,
{
    let mut guard = s
        .lock()
        .map_err(|e| anyhow::anyhow!("status lock poisoned: {}", e))?;
    let val = f(&mut *guard)?;
    Ok(val.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn smoke_read_write() {
        let s = Arc::new(Mutex::new(Status::new()));

        // read some field
        // read some field (inline selected logic from Status::selected)
        let sel: usize = read_status(&s, |st| {
            use crate::protocol::status::Page;
            let cur = st.ui.pages.last().cloned().unwrap_or_default();
            let selected = match cur {
                Page::Entry { selected, .. } => selected,
                Page::Modbus { selected, .. } => selected,
            };
            Ok(selected)
        })
        .unwrap();

        // write mutate selected (inline set_selected + sync_ui_from_page)
        let ret: () = write_status(&s, |st| {
            use crate::protocol::status::Page;
            // ensure a page exists (current_page_mut behaviour)
            if st.ui.pages.is_empty() {
                st.ui.pages.push(Page::default());
            }
            // set selected on the current page
            match st.ui.pages.last_mut().unwrap() {
                Page::Entry { selected, .. } => *selected = sel + 1,
                Page::Modbus { selected, .. } => *selected = sel + 1,
            }
            // inline sync_ui_from_page: copy fields from current page into flat ui
            let cur = st.ui.pages.last().cloned().unwrap_or_default();
            match cur {
                Page::Entry {
                    selected,
                    input_mode,
                    input_editing,
                    input_buffer,
                    app_mode,
                } => {
                    st.ui.selected = selected;
                    st.ui.input_mode = input_mode;
                    st.ui.input_editing = input_editing;
                    st.ui.input_buffer = input_buffer;
                    st.ui.app_mode = app_mode;
                    st.ui.subpage_active = false;
                    st.ui.subpage_form = None;
                }
                Page::Modbus {
                    selected,
                    subpage_active,
                    subpage_form,
                    subpage_tab_index,
                    logs,
                    log_selected,
                    log_view_offset,
                    log_auto_scroll,
                    log_clear_pending,
                    input_mode,
                    input_editing,
                    input_buffer,
                    app_mode,
                } => {
                    st.ui.selected = selected;
                    st.ui.subpage_active = subpage_active;
                    st.ui.subpage_form = subpage_form;
                    st.ui.subpage_tab_index = subpage_tab_index;
                    st.ui.logs = logs;
                    st.ui.log_selected = log_selected;
                    st.ui.log_view_offset = log_view_offset;
                    st.ui.log_auto_scroll = log_auto_scroll;
                    st.ui.log_clear_pending = log_clear_pending;
                    st.ui.input_mode = input_mode;
                    st.ui.input_editing = input_editing;
                    st.ui.input_buffer = input_buffer;
                    st.ui.app_mode = app_mode;
                }
            }
            Ok(())
        })
        .unwrap();

        let sel2: usize = read_status(&s, |st| {
            use crate::protocol::status::Page;
            let cur = st.ui.pages.last().cloned().unwrap_or_default();
            let selected = match cur {
                Page::Entry { selected, .. } => selected,
                Page::Modbus { selected, .. } => selected,
            };
            Ok(selected)
        })
        .unwrap();
        assert_eq!(sel2, sel + 1);
        let _ = ret;
    }
}
