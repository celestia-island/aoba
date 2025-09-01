use chrono::{DateTime, Local};
use std::{collections::HashMap, time::Duration};

use serialport::{SerialPort, SerialPortInfo};

use crate::protocol::tty::available_ports_sorted;

/// Parsed summary of a captured protocol request / response for UI display.
#[derive(Debug, Clone)]
pub struct ParsedRequest {
    /// origin of the message (e.g. "master-stack" or "main-stack")
    pub origin: String,
    /// "R" or "W"
    pub rw: String,
    /// textual command or function code (e.g. "Read Coils / 0x01")
    pub command: String,
    pub slave_id: u8,
    pub address: u16,
    pub length: u16,
}

/// A single captured log entry for UI presentation.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub when: DateTime<Local>,
    /// raw bytes or textual payload (displayed truncated)
    pub raw: String,
    /// optional parsed summary
    pub parsed: Option<ParsedRequest>,
}

/// Input mode for the log input box: ASCII text or Hex bytes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Ascii,
    Hex,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Parity {
    None,
    Even,
    Odd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterMode {
    Coils = 1,
    DiscreteInputs = 2,
    Holding = 3,
    Input = 4,
}

impl RegisterMode {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Coils,
            2 => Self::DiscreteInputs,
            3 => Self::Holding,
            4 => Self::Input,
            _ => Self::Coils,
        }
    }
    pub fn as_u8(self) -> u8 {
        self as u8
    }
    pub fn all() -> [RegisterMode; 4] {
        [
            Self::Coils,
            Self::DiscreteInputs,
            Self::Holding,
            Self::Input,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct RegisterEntry {
    pub slave_id: u8,
    pub mode: RegisterMode,
    pub address: u16,
    pub length: u16,
    /// Placeholder register values for UI editing (length-sized, hex display)
    pub values: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct SubpageForm {
    pub baud: u32,
    pub parity: Parity,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub registers: Vec<RegisterEntry>,
    // UI state
    pub cursor: usize, // which field or register is focused
    pub editing: bool, // whether in edit mode
    // which specific field is being edited (None when not editing)
    pub editing_field: Option<EditingField>,
    // input buffer for the current editing session (text)
    pub input_buffer: String,
    /// temporary index used when editing a multi-option field (like Baud presets + Custom)
    pub edit_choice_index: Option<usize>,
    /// whether we've entered the deeper confirm / editing stage for a choice (e.g. Custom baud)
    pub edit_confirmed: bool,
    // --- Master list (tab 1) 专用 UI 状态 ---
    /// Cursor in master list panel (points to a master or the trailing "new" entry)
    pub master_cursor: usize,
    /// Whether currently editing a master entry (deprecated flag, kept for future use)
    pub master_editing: bool,
    /// Whether a specific master field is selected (field selection layer)
    pub master_field_selected: bool,
    /// Whether the current field is in active input editing (vs merely selected)
    pub master_field_editing: bool,
    /// Currently focused master field
    pub master_edit_field: Option<MasterEditField>,
    /// Input buffer for master field editing (hex / decimal)
    pub master_input_buffer: String,
    /// Index of the master being edited
    pub master_edit_index: Option<usize>,
}

impl Default for SubpageForm {
    fn default() -> Self {
        Self {
            baud: 9600,
            parity: Parity::None,
            data_bits: 8,
            stop_bits: 1,
            registers: vec![],
            cursor: 0,
            editing: false,
            editing_field: None,
            input_buffer: String::new(),
            edit_choice_index: None,
            edit_confirmed: false,
            master_cursor: 0,
            master_editing: false,
            master_field_selected: false,
            master_field_editing: false,
            master_edit_field: None,
            master_input_buffer: String::new(),
            master_edit_index: None,
        }
    }
}

/// Which concrete field inside the SubpageForm is currently being edited.
#[derive(Debug, Clone)]
pub enum EditingField {
    Baud,
    Parity,
    DataBits,
    StopBits,
    RegisterField { idx: usize, field: RegisterField },
}

#[derive(Debug, Clone)]
pub enum RegisterField {
    SlaveId,
    Mode,
    Address,
    Length,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MasterEditField {
    Id,
    Type,
    Start,
    End,
    /// Single register value (absolute address)
    Value(u16),
}

// Focus enum removed: UI now uses single-pane left list only

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RightMode {
    Master,
    SlaveStack,
    Listen,
}

#[derive(Debug)]
pub struct Status {
    pub ports: Vec<SerialPortInfo>,
    /// occupancy state for each port (same index as `ports`)
    pub port_states: Vec<PortState>,
    /// optional open handle when this app occupies the port
    pub port_handles: Vec<Option<Box<dyn SerialPort>>>,
    pub selected: usize,
    pub auto_refresh: bool,
    pub last_refresh: Option<DateTime<Local>>,
    pub error: Option<(String, DateTime<Local>)>,
    pub right_mode: RightMode,
    /// When Some, a subpage for the right side is active (entered). None means main entry view.
    pub active_subpage: Option<RightMode>,
    /// transient UI state for the active subpage (editable form)
    pub subpage_form: Option<SubpageForm>,
    /// selected tab index inside the active right-side subpage
    pub subpage_tab_index: usize,
    /// transient mode selector overlay state
    pub mode_selector_active: bool,
    pub mode_selector_index: usize,
    /// recent protocol / log entries for display in log panel
    pub logs: Vec<LogEntry>,
    /// index of selected log entry when viewing logs (visual groups)
    pub log_selected: usize,
    /// offset of the visible log group: index of the bottom-most visible group in the viewport
    /// (renderer computes the top from this bottom index and the visible page size)
    pub log_view_offset: usize,
    /// whether log view auto-scrolls to bottom when new entries arrive
    pub log_auto_scroll: bool,
    /// input mode for the log input area
    pub input_mode: InputMode,
    /// whether the log input is currently in editing state
    pub input_editing: bool,
    /// input buffer for the log input area
    pub input_buffer: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortState {
    Free,
    OccupiedByThis,
    OccupiedByOther,
}

impl Status {
    pub fn new() -> Self {
        let ports = available_ports_sorted();
        let port_states = Self::detect_port_states(&ports);
        let port_handles = ports.iter().map(|_| None).collect();
        Self {
            ports,
            port_states,
            port_handles,
            selected: 0,

            auto_refresh: true,
            last_refresh: None,
            error: None,
            right_mode: RightMode::Master,
            active_subpage: None,
            subpage_form: None,
            subpage_tab_index: 0,
            mode_selector_active: false,
            mode_selector_index: 0,
            logs: Vec::new(),
            log_selected: 0,
            log_view_offset: 0,
            log_auto_scroll: true,
            input_mode: InputMode::Ascii,
            input_editing: false,
            input_buffer: String::new(),
        }
    }

    /// Adjust the log view window according to the current terminal height so the selected entry stays visible.
    pub fn adjust_log_view(&mut self, term_height: u16) {
        if self.logs.is_empty() {
            return;
        }
        let bottom_len = if self.error.is_some() || self.active_subpage.is_some() {
            2
        } else {
            1
        };
        // The constant 5 matches empirically reserved rows (title / input etc.) in current TUI layout; extract if layout changes.
        let logs_area_h = (term_height as usize).saturating_sub(bottom_len + 5);
        let inner_h = logs_area_h.saturating_sub(2);
        let groups_per_screen = std::cmp::max(
            1usize,
            inner_h / crate::tui::utils::constants::LOG_GROUP_HEIGHT,
        );
        let bottom = if self.log_auto_scroll {
            self.logs.len().saturating_sub(1)
        } else {
            std::cmp::min(self.log_view_offset, self.logs.len().saturating_sub(1))
        };
        let top = if bottom + 1 >= groups_per_screen {
            bottom + 1 - groups_per_screen
        } else {
            0
        };
        if self.log_selected < top {
            self.log_auto_scroll = false;
            let half = groups_per_screen / 2;
            let new_bottom =
                std::cmp::min(self.logs.len().saturating_sub(1), self.log_selected + half);
            self.log_view_offset = new_bottom;
        } else if self.log_selected > bottom {
            self.log_auto_scroll = false;
            self.log_view_offset = self.log_selected;
        }
    }

    /// Page up in the log (decrease bottom index).
    pub fn page_up(&mut self, page: usize) {
        if self.logs.is_empty() {
            return;
        }
        if self.log_view_offset > page {
            self.log_view_offset = self.log_view_offset.saturating_sub(page);
        } else {
            self.log_view_offset = 0;
        }
        self.log_auto_scroll = false;
    }

    /// Page down in the log (increase bottom index).
    pub fn page_down(&mut self, page: usize) {
        if self.logs.is_empty() {
            return;
        }
        let total = self.logs.len();
        self.log_view_offset = std::cmp::min(total - 1, self.log_view_offset.saturating_add(page));
        self.log_auto_scroll = false;
    }

    /// Initialize subpage_form from selected port when entering a subpage
    pub fn init_subpage_form(&mut self) {
        // If no ports or selected is virtual, create a default form
        if self.ports.is_empty() || self.selected >= self.ports.len() {
            self.subpage_form = Some(SubpageForm::default());
            return;
        }
        // Try to populate from existing open handle if available
        let mut form = SubpageForm::default();
        if let Some(slot) = self.port_handles.get(self.selected) {
            if let Some(handle) = slot.as_ref() {
                form.baud = handle.baud_rate().unwrap_or(9600);
                form.stop_bits = handle
                    .stop_bits()
                    .map(|s| match s {
                        serialport::StopBits::One => 1,
                        serialport::StopBits::Two => 2,
                    })
                    .unwrap_or(1);
                // parity mapping
                if let Ok(p) = handle.parity() {
                    form.parity = match p {
                        serialport::Parity::None => Parity::None,
                        serialport::Parity::Even => Parity::Even,
                        serialport::Parity::Odd => Parity::Odd,
                    };
                }
            }
        }
        self.subpage_form = Some(form);
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.error = Some((msg.into(), Local::now()));
    }

    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// Create an App with provided ports (useful for tests)
    #[cfg(test)]
    pub fn with_ports(ports: Vec<SerialPortInfo>) -> Self {
        let port_states = ports.iter().map(|_| PortState::Free).collect();
        let port_handles = ports.iter().map(|_| None).collect();
        Self {
            ports,
            port_states,
            port_handles,
            selected: 0,

            auto_refresh: false,
            last_refresh: None,
            error: None,
            right_mode: RightMode::Master,
            active_subpage: None,
            subpage_form: None,
            subpage_tab_index: 0,
            mode_selector_active: false,
            mode_selector_index: 0,
            logs: Vec::new(),
            log_selected: 0,
            log_view_offset: 0,
            log_auto_scroll: true,
            input_mode: InputMode::Ascii,
            input_editing: false,
            input_buffer: String::new(),
        }
    }

    /// Append a log entry to the internal buffer (caps at 1000 entries)
    pub fn append_log(&mut self, entry: LogEntry) {
        const MAX: usize = 1000;
        self.logs.push(entry);
        if self.logs.len() > MAX {
            let excess = self.logs.len() - MAX;
            self.logs.drain(0..excess);
            // ensure selected index remains valid
            if self.log_selected >= self.logs.len() {
                self.log_selected = self.logs.len().saturating_sub(1);
            }
        }
        // maintain auto-scroll behaviour: when auto-scroll enabled, keep view anchored to the latest
        if self.log_auto_scroll {
            // position the view offset so bottom aligns with last entry (we'll compute exact top in renderer)
            if self.logs.is_empty() {
                self.log_view_offset = 0;
            } else {
                self.log_view_offset = self.logs.len().saturating_sub(1);
                // If auto-scroll is enabled, also move the selection to the newest entry
                self.log_selected = self.logs.len().saturating_sub(1);
            }
        }
    }

    /// Re-scan available ports and reset selection if needed
    pub fn refresh(&mut self) {
        let new_ports = available_ports_sorted();
        // Remember previously selected port name (if any real port selected)
        let prev_selected_name = if !self.ports.is_empty() && self.selected < self.ports.len() {
            Some(self.ports[self.selected].port_name.clone())
        } else {
            None
        };
        // Preserve known states and handles by port name
        let mut name_to_state: HashMap<String, PortState> = HashMap::new();
        let mut name_to_handle: HashMap<String, Option<Box<dyn SerialPort>>> = HashMap::new();
        for (i, p) in self.ports.iter().enumerate() {
            if let Some(s) = self.port_states.get(i) {
                name_to_state.insert(p.port_name.clone(), *s);
            }
            // take ownership of existing handle if any
            if let Some(h) = self.port_handles.get_mut(i) {
                let taken = h.take();
                name_to_handle.insert(p.port_name.clone(), taken);
            }
        }
        self.ports = new_ports;
        // Rebuild port_states and port_handles preserving by name
        let mut new_states: Vec<PortState> = Vec::with_capacity(self.ports.len());
        let mut new_handles: Vec<Option<Box<dyn SerialPort>>> =
            Vec::with_capacity(self.ports.len());
        for p in self.ports.iter() {
            if let Some(s) = name_to_state.remove(&p.port_name) {
                new_states.push(s);
            } else if Self::is_port_free(&p.port_name) {
                new_states.push(PortState::Free);
            } else {
                new_states.push(PortState::OccupiedByOther);
            }
            // move back handle if existed
            if let Some(h) = name_to_handle.remove(&p.port_name) {
                new_handles.push(h);
            } else {
                new_handles.push(None);
            }
        }
        self.port_states = new_states;
        self.port_handles = new_handles;
        if self.ports.is_empty() {
            // No real ports -> reset selection to 0 (no virtual items rendered)
            self.selected = 0;
        } else {
            // try to restore previous selected port by name
            if let Some(name) = prev_selected_name {
                if let Some(idx) = self.ports.iter().position(|p| p.port_name == name) {
                    self.selected = idx;
                }
            }
            // ensure selected is within allowed range: real ports + 2 virtual items
            let total = self.ports.len().saturating_add(2);
            if self.selected >= total {
                self.selected = 0;
            }
        }
        self.last_refresh = Some(Local::now());
    }

    fn is_port_free(port_name: &str) -> bool {
        // Try to open the port briefly; if succeed it's free (we immediately drop it)
        match serialport::new(port_name, 9600)
            .timeout(Duration::from_millis(50))
            .open()
        {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    fn detect_port_states(ports: &Vec<SerialPortInfo>) -> Vec<PortState> {
        ports
            .iter()
            .map(|p| {
                if Self::is_port_free(&p.port_name) {
                    PortState::Free
                } else {
                    PortState::OccupiedByOther
                }
            })
            .collect()
    }

    /// Toggle the selected port's occupancy by this app. No-op if other program occupies the port.
    pub fn toggle_selected_port(&mut self) {
        if self.ports.is_empty() {
            return;
        }
        let i = self.selected;
        // if selected is beyond real ports, handle virtual items
        let special_base = self.ports.len();
        if i >= special_base {
            let rel = i - special_base;
            if rel == 0 {
                // Refresh
                self.refresh();
            } else {
                // Manual specify: not implemented here; set an info / error
                self.set_error(
                    "Manual device specify: only supported on Linux and not implemented yet",
                );
            }
            return;
        }
        if let Some(state) = self.port_states.get_mut(i) {
            match state {
                PortState::Free => {
                    // try to open and hold the port
                    let port_name = self.ports[i].port_name.clone();
                    match serialport::new(&port_name, 9600)
                        .timeout(Duration::from_millis(200))
                        .open()
                    {
                        Ok(handle) => {
                            if let Some(hslot) = self.port_handles.get_mut(i) {
                                *hslot = Some(handle);
                            }
                            *state = PortState::OccupiedByThis;
                        }
                        Err(e) => {
                            // cannot open -> likely occupied by other
                            *state = PortState::OccupiedByOther;
                            self.set_error(format!("failed to open {}: {}", port_name, e));
                        }
                    }
                }
                PortState::OccupiedByThis => {
                    // drop handle
                    if let Some(hslot) = self.port_handles.get_mut(i) {
                        *hslot = None;
                    }
                    *state = PortState::Free;
                }
                PortState::OccupiedByOther => {
                    // don't change
                }
            }
        }
    }

    pub fn toggle_auto_refresh(&mut self) {
        self.auto_refresh = !self.auto_refresh;
    }
    pub fn next(&mut self) {
        // Navigate among real ports only
        let total = self.ports.len();
        if total > 0 {
            self.selected = (self.selected + 1) % total;
        }
    }

    pub fn prev(&mut self) {
        let total = self.ports.len();
        if total == 0 {
            return;
        }
        if self.selected == 0 {
            self.selected = total - 1;
        } else {
            self.selected -= 1;
        }
    }

    /// Navigate among visual rows in the left pane including the two trailing virtual items
    /// (Refresh and Manual specify). This is used by the TUI navigation so the user can
    /// select those bottom options even though the logical model's next()/prev() operate on
    /// real ports only for test stability.
    pub fn next_visual(&mut self) {
        let total = self.ports.len().saturating_add(2);
        if total > 0 {
            self.selected = (self.selected + 1) % total;
        }
    }

    pub fn prev_visual(&mut self) {
        let total = self.ports.len().saturating_add(2);
        if total == 0 {
            return;
        }
        if self.selected == 0 {
            self.selected = total - 1;
        } else {
            self.selected -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serialport::{SerialPortInfo, SerialPortType};

    fn fake_port(name: &str) -> SerialPortInfo {
        SerialPortInfo {
            port_name: name.to_string(),
            port_type: SerialPortType::Unknown,
        }
    }

    #[test]
    fn test_navigation() {
        let ports = vec![fake_port("COM1"), fake_port("COM2")];
        let mut app = Status::with_ports(ports);
        assert_eq!(app.selected, 0);
        app.next();
        assert_eq!(app.selected, 1);
        app.next();
        assert_eq!(app.selected, 0);
        app.prev();
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn test_focus_and_refresh() {
        let ports = vec![fake_port("COM1")];
        let mut app = Status::with_ports(ports);
        // Call refresh (may change ports depending on environment)
        app.refresh();
        // Ensure selected is in bounds
        if !app.ports.is_empty() {
            assert!(app.selected < app.ports.len());
        }
    }

    #[test]
    fn test_log_paging_and_adjust() {
        let mut s = Status::with_ports(vec![]);
        // Fill 20 log entries.
        for i in 0..20 {
            s.append_log(LogEntry {
                when: Local::now(),
                raw: format!("payload{i}"),
                parsed: None,
            });
        }
        // Initial auto_scroll should be anchored at the end.
        assert_eq!(s.log_view_offset, 19);
        s.page_up(5);
        assert!(s.log_view_offset <= 19);
        let prev = s.log_view_offset;
        s.page_down(5);
        assert!(s.log_view_offset >= prev);
        // Select the first entry; adjusting the view should move the offset upward (no longer stuck to bottom).
        s.log_selected = 0;
        s.adjust_log_view(40); // Simulated terminal height.
        assert!(s.log_view_offset <= 19);
    }
}
