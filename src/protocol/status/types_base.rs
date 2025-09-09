use chrono::{DateTime, Local};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};
use yuuka::derive_struct;

use rmodbus::server::storage::ModbusStorageSmall;
use serialport::SerialPortInfo;

use crate::protocol::{
    runtime::PortRuntimeHandle,
    status::{LogEntry, SubpageForm},
    tty::PortExtra,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Ascii,
    Hex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubpageTab {
    Config = 0,
    Body = 1,
    Log = 2,
}

impl SubpageTab {
    /// Return the numeric index for UI widgets that require usize.
    pub fn as_usize(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeOverlayIndex {
    Modbus = 0,
    Mqtt = 1,
}

impl ModeOverlayIndex {
    /// Return the numeric index for UI widgets that require usize.
    pub fn as_usize(self) -> usize {
        self as usize
    }
}

impl Status {
    /// Return the current/top page by value (cloned) to avoid lifetime issues.
    pub fn current_page(&self) -> Page {
        self.ui.pages.last().cloned().unwrap_or_default()
    }

    /// Return a mutable reference to the current/top page, creating a default
    /// page if the stack is empty.
    pub fn current_page_mut(&mut self) -> &mut Page {
        if self.ui.pages.is_empty() {
            self.ui.pages.push(Page::default());
        }
        self.ui.pages.last_mut().unwrap()
    }

    /// Push a new page onto the stack and sync flat ui fields from it.
    pub fn push_page(&mut self, page: Page) {
        self.ui.pages.push(page);
        self.sync_ui_from_page();
    }

    /// Pop the top page and sync flat ui fields from the new top page.
    pub fn pop_page(&mut self) -> Option<Page> {
        let p = self.ui.pages.pop();
        self.sync_ui_from_page();
        p
    }

    /// Sync the flat `ui` fields from the current page so existing code that
    /// reads `ui.*` continues to work.
    pub fn sync_ui_from_page(&mut self) {
        match self.current_page().clone() {
            Page::Entry {
                selected,
                input_mode,
                input_editing,
                input_buffer,
                app_mode,
            } => {
                self.ui.selected = selected;
                self.ui.input_mode = input_mode;
                self.ui.input_editing = input_editing;
                self.ui.input_buffer = input_buffer;
                self.ui.app_mode = app_mode;
                // entry page doesn't use subpage_* fields
                self.ui.subpage_active = false;
                self.ui.subpage_form = None;
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
                self.ui.selected = selected;
                self.ui.subpage_active = subpage_active;
                self.ui.subpage_form = subpage_form;
                self.ui.subpage_tab_index = subpage_tab_index;
                self.ui.logs = logs;
                self.ui.log_selected = log_selected;
                self.ui.log_view_offset = log_view_offset;
                self.ui.log_auto_scroll = log_auto_scroll;
                self.ui.log_clear_pending = log_clear_pending;
                self.ui.input_mode = input_mode;
                self.ui.input_editing = input_editing;
                self.ui.input_buffer = input_buffer;
                self.ui.app_mode = app_mode;
            }
        }
    }

    /// Update the current page's fields from the flat `ui` fields. Useful
    /// when older code mutates the flat fields and you want the page stack to
    /// reflect those changes.
    pub fn sync_page_from_ui(&mut self) {
        if self.ui.pages.is_empty() {
            self.ui.pages.push(Page::default());
        }
        match self.ui.pages.last_mut().unwrap() {
            Page::Entry {
                selected,
                input_mode,
                input_editing,
                input_buffer,
                app_mode,
            } => {
                *selected = self.ui.selected;
                *input_mode = self.ui.input_mode;
                *input_editing = self.ui.input_editing;
                *input_buffer = self.ui.input_buffer.clone();
                *app_mode = self.ui.app_mode;
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
                *selected = self.ui.selected;
                *subpage_active = self.ui.subpage_active;
                *subpage_form = self.ui.subpage_form.clone();
                *subpage_tab_index = self.ui.subpage_tab_index;
                *logs = self.ui.logs.clone();
                *log_selected = self.ui.log_selected;
                *log_view_offset = self.ui.log_view_offset;
                *log_auto_scroll = self.ui.log_auto_scroll;
                *log_clear_pending = self.ui.log_clear_pending;
                *input_mode = self.ui.input_mode;
                *input_editing = self.ui.input_editing;
                *input_buffer = self.ui.input_buffer.clone();
                *app_mode = self.ui.app_mode;
            }
        }
    }

    // --- Page-stack-first accessors / modifiers ---

    pub fn selected(&self) -> usize {
        match self.current_page() {
            Page::Entry { selected, .. } => selected,
            Page::Modbus { selected, .. } => selected,
        }
    }

    pub fn set_selected(&mut self, v: usize) {
        match self.current_page_mut() {
            Page::Entry { selected, .. } => *selected = v,
            Page::Modbus { selected, .. } => *selected = v,
        }
        self.sync_ui_from_page();
    }

    pub fn auto_refresh(&self) -> bool {
        self.ui.auto_refresh
    }

    pub fn set_auto_refresh(&mut self, v: bool) {
        self.ui.auto_refresh = v;
    }

    pub fn last_refresh(&self) -> Option<DateTime<Local>> {
        self.ui.last_refresh
    }

    pub fn set_last_refresh(&mut self, t: Option<DateTime<Local>>) {
        self.ui.last_refresh = t;
    }

    pub fn error(&self) -> Option<&(String, DateTime<Local>)> {
        self.ui.error.as_ref()
    }

    pub fn set_error_msg<T: Into<String>>(&mut self, msg: T) {
        self.ui.error = Some((msg.into(), Local::now()));
    }

    // Note: `clear_error` implementation exists in `port.rs`; avoid duplicate.

    pub fn subpage_active(&self) -> bool {
        match self.current_page() {
            Page::Modbus { subpage_active, .. } => subpage_active,
            _ => false,
        }
    }

    pub fn set_subpage_active(&mut self, v: bool) {
        if let Page::Modbus { subpage_active, .. } = self.current_page_mut() {
            *subpage_active = v;
        }
        self.sync_ui_from_page();
    }

    pub fn subpage_form(&self) -> Option<SubpageForm> {
        match self.current_page() {
            Page::Modbus { subpage_form, .. } => subpage_form,
            _ => None,
        }
    }

    pub fn set_subpage_form(&mut self, f: Option<SubpageForm>) {
        if let Page::Modbus { subpage_form, .. } = self.current_page_mut() {
            *subpage_form = f;
        }
        self.sync_ui_from_page();
    }

    pub fn subpage_tab_index(&self) -> SubpageTab {
        match self.current_page() {
            Page::Modbus {
                subpage_tab_index, ..
            } => subpage_tab_index,
            _ => SubpageTab::Config,
        }
    }

    pub fn set_subpage_tab_index(&mut self, idx: SubpageTab) {
        if let Page::Modbus {
            subpage_tab_index, ..
        } = self.current_page_mut()
        {
            *subpage_tab_index = idx;
        }
        self.sync_ui_from_page();
    }

    pub fn logs(&self) -> Vec<LogEntry> {
        match self.current_page() {
            Page::Modbus { logs, .. } => logs,
            _ => self.ui.logs.clone(),
        }
    }

    pub fn append_log_entry(&mut self, entry: LogEntry) {
        // reuse existing append logic (there's a similar fn in port.rs),
        // operate on flat fields for now and keep page in sync
        const MAX: usize = 1000;
        self.ui.logs.push(entry);
        if self.ui.logs.len() > MAX {
            let excess = self.ui.logs.len() - MAX;
            self.ui.logs.drain(0..excess);
            if self.ui.log_selected >= self.ui.logs.len() {
                self.ui.log_selected = self.ui.logs.len().saturating_sub(1);
            }
        }
        if self.ui.log_auto_scroll {
            if self.ui.logs.is_empty() {
                self.ui.log_view_offset = 0;
            } else {
                self.ui.log_view_offset = self.ui.logs.len().saturating_sub(1);
                self.ui.log_selected = self.ui.logs.len().saturating_sub(1);
            }
        }
        self.sync_page_from_ui();
    }

    pub fn input_mode(&self) -> InputMode {
        match self.current_page() {
            Page::Entry { input_mode, .. } => input_mode,
            Page::Modbus { input_mode, .. } => input_mode,
        }
    }

    pub fn set_input_mode(&mut self, m: InputMode) {
        match self.current_page_mut() {
            Page::Entry { input_mode, .. } => *input_mode = m,
            Page::Modbus { input_mode, .. } => *input_mode = m,
        }
        self.sync_ui_from_page();
    }

    pub fn input_editing(&self) -> bool {
        match self.current_page() {
            Page::Entry { input_editing, .. } => input_editing,
            Page::Modbus { input_editing, .. } => input_editing,
        }
    }

    pub fn set_input_editing(&mut self, v: bool) {
        match self.current_page_mut() {
            Page::Entry { input_editing, .. } => *input_editing = v,
            Page::Modbus { input_editing, .. } => *input_editing = v,
        }
        self.sync_ui_from_page();
    }

    pub fn input_buffer(&self) -> String {
        match self.current_page() {
            Page::Entry { input_buffer, .. } => input_buffer,
            Page::Modbus { input_buffer, .. } => input_buffer,
        }
    }

    pub fn set_input_buffer<S: Into<String>>(&mut self, s: S) {
        let s = s.into();
        match self.current_page_mut() {
            Page::Entry { input_buffer, .. } => *input_buffer = s.clone(),
            Page::Modbus { input_buffer, .. } => *input_buffer = s.clone(),
        }
        self.sync_ui_from_page();
    }

    pub fn app_mode(&self) -> AppMode {
        match self.current_page() {
            Page::Entry { app_mode, .. } => app_mode,
            Page::Modbus { app_mode, .. } => app_mode,
        }
    }

    pub fn set_app_mode(&mut self, m: AppMode) {
        match self.current_page_mut() {
            Page::Entry { app_mode, .. } => *app_mode = m,
            Page::Modbus { app_mode, .. } => *app_mode = m,
        }
        self.sync_ui_from_page();
    }

    pub fn mode_overlay_active(&self) -> bool {
        self.ui.mode_overlay_active
    }

    pub fn set_mode_overlay_active(&mut self, v: bool) {
        self.ui.mode_overlay_active = v;
    }

    pub fn mode_overlay_index(&self) -> ModeOverlayIndex {
        self.ui.mode_overlay_index
    }

    pub fn set_mode_overlay_index(&mut self, idx: ModeOverlayIndex) {
        self.ui.mode_overlay_index = idx;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Modbus,
    Mqtt,
}

derive_struct! {
    pub Status {
        ports: {
            list: Vec<SerialPortInfo>,
            extras: Vec<PortExtra>,
            states: Vec<crate::protocol::status::PortState>,
            handles: Vec<Option<crate::protocol::status::SerialPortWrapper>>,
            runtimes: Vec<Option<PortRuntimeHandle>>,
            about_view_offset: usize,
        },
        ui: {
            selected: usize,
            auto_refresh: bool,
            last_refresh: Option<DateTime<Local>>,
            error: Option<(String, DateTime<Local>)>,
            subpage_active: bool,
            subpage_form: Option<SubpageForm>,
            subpage_tab_index: SubpageTab = SubpageTab::Config,
            logs: Vec<LogEntry>,
            log_selected: usize,
            log_view_offset: usize,
            log_auto_scroll: bool,
            log_clear_pending: bool,
            input_mode: InputMode = InputMode::Ascii,
            input_editing: bool,
            input_buffer: String,
            app_mode: AppMode = AppMode::Modbus,
            mode_overlay_active: bool,
            mode_overlay_index: ModeOverlayIndex = ModeOverlayIndex::Modbus,
            pages: [enum Page {
                Entry {
                    selected: usize,
                    input_mode: InputMode,
                    input_editing: bool,
                    input_buffer: String,
                    app_mode: AppMode,
                },
                Modbus {
                    // TODO: Subpage level enum
                    selected: usize,
                    subpage_active: bool,
                    subpage_form: Option<SubpageForm>,
                    subpage_tab_index: SubpageTab,
                    logs: Vec<LogEntry>,
                    log_selected: usize,
                    log_view_offset: usize,
                    log_auto_scroll: bool,
                    log_clear_pending: bool,
                    input_mode: InputMode,
                    input_editing: bool,
                    input_buffer: String,
                    app_mode: AppMode,
                },
            }] = vec![Page::Entry {
                selected: 0,
                input_mode: InputMode::Ascii,
                input_editing: false,
                input_buffer: String::new(),
                app_mode: AppMode::Modbus,
            }],
        },
        per_port: {
            states: HashMap<String, crate::protocol::status::PerPortState>,
            slave_contexts: HashMap<String, Arc<Mutex<ModbusStorageSmall>>>,
            pending_sync_port: Option<String>,
        },
        scan: {
            last_scan_info: Vec<String>,
            last_scan_time: Option<DateTime<Local>>,
        },
        busy: {
            busy: bool = false,
            spinner_frame: u8 = 0,
            polling_paused: bool = false,
        },
        toggles: {
            last_port_toggle: Option<std::time::Instant>,
            port_toggle_min_interval_ms: u64 = crate::protocol::status::PORT_TOGGLE_MIN_INTERVAL_MS,
        },
        recent: {
            auto_sent: VecDeque<(Vec<u8>, std::time::Instant)>,
            auto_requests: VecDeque<(Vec<u8>, std::time::Instant)>,
        },
    }
}
