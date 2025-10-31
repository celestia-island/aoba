use serde::{Deserialize, Serialize};

use crate::status::types;

/// A small enum used to represent temporary input buffers across the UI.
/// - `None` means there's no active temporary buffer
/// - `Index(n)` records a selected index for selectors
/// - `String(bytes)` stores raw bytes when editing free-form input
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum InputRawBuffer {
    #[default]
    None,
    Index(usize),
    /// String buffer with an editing cursor offset (signed). Offset semantics:
    /// - offset >= 0: character index from start (0..=len)
    /// - offset < 0: character index from end (len as isize + offset), clamped
    String {
        bytes: Vec<u8>,
        offset: isize,
    },
}

impl std::fmt::Display for InputRawBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputRawBuffer::None => write!(f, ""),
            InputRawBuffer::Index(i) => write!(f, "{i}"),
            InputRawBuffer::String { bytes, .. } => match std::str::from_utf8(bytes) {
                Ok(s) => write!(f, "{s}"),
                Err(_) => write!(f, "{bytes:?}"),
            },
        }
    }
}

impl From<usize> for InputRawBuffer {
    fn from(i: usize) -> Self {
        InputRawBuffer::Index(i)
    }
}

impl InputRawBuffer {
    /// Return true if buffer contains no useful content.
    pub fn is_empty(&self) -> bool {
        match self {
            InputRawBuffer::None => true,
            InputRawBuffer::Index(_) => false,
            InputRawBuffer::String { bytes, .. } => bytes.is_empty(),
        }
    }

    /// Clear the buffer (becomes None)
    pub fn clear(&mut self) {
        *self = InputRawBuffer::None;
    }

    /// Push a char into the string buffer, creating one if necessary.
    pub fn push(&mut self, c: char) {
        match self {
            InputRawBuffer::String { bytes, offset } => {
                // Insert char at current cursor offset (character index semantics)
                let mut s = String::from_utf8_lossy(bytes).to_string();
                let len_chars = s.chars().count() as isize;
                // compute insertion position
                let mut pos = if *offset >= 0 {
                    *offset
                } else {
                    len_chars + *offset
                };
                if pos < 0 {
                    pos = 0;
                }
                if pos > len_chars {
                    pos = len_chars;
                }
                let insert_pos = pos as usize;
                s.insert(insert_pos, c);
                *bytes = s.into_bytes();
                // advance cursor after inserted char
                if *offset >= 0 {
                    *offset += 1;
                } else {
                    // keep negative offsets relative to end by no change
                }
            }
            _ => {
                let mut v = Vec::new();
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                v.extend_from_slice(s.as_bytes());
                *self = InputRawBuffer::String {
                    bytes: v,
                    offset: 1,
                };
            }
        }
    }

    /// Pop the last character from the string buffer (if any).
    pub fn pop(&mut self) -> Option<char> {
        match self {
            InputRawBuffer::String { bytes, offset } => {
                if let Ok(s) = String::from_utf8(bytes.clone()) {
                    let len_chars = s.chars().count() as isize;
                    // determine deletion index: character before cursor
                    let pos = if *offset >= 0 {
                        *offset
                    } else {
                        len_chars + *offset
                    };
                    if pos <= 0 {
                        return None;
                    }
                    let del_pos = (pos - 1) as usize;
                    // remove char at del_pos
                    let mut chars: Vec<char> = s.chars().collect();
                    let ch = chars.get(del_pos).copied();
                    if ch.is_some() {
                        chars.remove(del_pos);
                        let new_s: String = chars.into_iter().collect();
                        *bytes = new_s.into_bytes();
                        // move cursor left when it was >=0
                        if *offset >= 0 {
                            *offset -= 1;
                            if *offset < 0 {
                                *offset = 0;
                            }
                        }
                    }
                    ch
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Return an owned Vec<char> of the internal string (or empty vec).
    pub fn chars(&self) -> Vec<char> {
        self.as_string().chars().collect()
    }

    /// Return an owned String representation of the buffer
    pub fn as_string(&self) -> String {
        match self {
            InputRawBuffer::String { bytes, .. } => String::from_utf8_lossy(bytes).into_owned(),
            InputRawBuffer::Index(i) => i.to_string(),
            InputRawBuffer::None => String::new(),
        }
    }

    /// Move cursor offset by delta (can be negative). Clamped to valid range.
    pub fn move_offset(&mut self, delta: isize) {
        if let InputRawBuffer::String { bytes, offset } = self {
            let s = String::from_utf8_lossy(bytes).to_string();
            let len_chars = s.chars().count() as isize;
            let mut new = *offset + delta;
            // clamp: allow negative values down to -len_chars
            if new < -len_chars {
                new = -len_chars;
            }
            if new > len_chars {
                new = len_chars;
            }
            *offset = new;
        }
    }

    /// Set the string buffer from a given String and set cursor offset to end.
    pub fn set_string_and_place_cursor_at_end(&mut self, s: String) {
        let len_chars = s.chars().count() as isize;
        *self = InputRawBuffer::String {
            bytes: s.into_bytes(),
            offset: len_chars,
        }
    }
}

/// Special entries that appear after the serial ports list in the Entry page.
/// Kept as a UI enum so other UI modules can reference the same canonical variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialEntry {
    Refresh,
    ManualSpecify,
    About,
}

impl SpecialEntry {
    pub const fn all() -> &'static [SpecialEntry] {
        &[
            SpecialEntry::Refresh,
            SpecialEntry::ManualSpecify,
            SpecialEntry::About,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    Ascii,
    Hex,
}

// UI-only editing enums (moved from types::modbus)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditingField {
    Loop,
    Baud,
    Parity,
    StopBits,
    DataBits,
    GlobalInterval,
    GlobalTimeout,
    RegisterField { index: usize, field: RegisterField },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterField {
    SlaveId,
    Mode,
    Address,
    Length,
    Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MasterEditField {
    Role,
    Id,
    Type,
    Start,
    End,
    Counter,
    Value(u16),
}

// AppMode is a small UI-oriented enum (moved here from modbus.rs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppMode {
    #[default]
    Modbus,
    Mqtt,
}

impl AppMode {
    pub fn as_usize(self) -> usize {
        match self {
            AppMode::Modbus => 0,
            AppMode::Mqtt => 1,
        }
    }
}

/// Snapshot structs for page-specific state to be passed into render/handler
/// functions so callers don't need to inspect the whole `Status` repeatedly.
#[derive(Debug, Clone)]
pub struct ModbusConfigStatus {
    pub selected_port: usize,

    pub edit_active: bool,
    pub edit_port: Option<String>,
    pub edit_field_index: usize,
    pub edit_field_key: Option<String>,
    pub edit_buffer: String,
    pub edit_cursor_pos: usize,
}

#[derive(Debug, Clone)]
pub struct ModbusDashboardStatus {
    pub selected_port: usize,

    pub cursor: usize,
    pub editing_field: Option<EditingField>,
    pub input_buffer: String,
    pub edit_choice_index: Option<usize>,
    pub edit_confirmed: bool,

    pub master_cursor: usize,
    pub master_field_selected: bool,
    pub master_field_editing: bool,
    pub master_edit_field: Option<MasterEditField>,
    pub master_edit_index: Option<usize>,
    pub master_input_buffer: String,
    pub poll_round_index: usize,
    pub in_flight_reg_index: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ModbusLogStatus {
    pub selected_port: usize,
}

#[derive(Debug, Clone)]
pub struct AboutStatus {
    pub view_offset: usize,
}

#[derive(Debug, Clone)]
pub struct EntryStatus {
    pub cursor: Option<types::cursor::EntryCursor>,
}
