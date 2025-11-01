use log::warn;
use once_cell::sync::Lazy;
use parking_lot::Mutex;

/// Placeholder value type for screenshot generation
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceholderValue {
    /// Decimal number (e.g., 123 -> {{#x}})
    Decimal(u16),
    /// Hexadecimal number (e.g., 0x1234 -> {{0x#x}})
    Hex(u16),
    /// Boolean value (e.g., ON/OFF -> {{0b#x}})
    Boolean(bool),
}

impl PlaceholderValue {
    /// Get the actual value as a string
    pub fn as_string(&self) -> String {
        match self {
            PlaceholderValue::Decimal(v) => format!("{}", v),
            PlaceholderValue::Hex(v) => format!("0x{:04X}", v),
            PlaceholderValue::Boolean(b) => if *b { "ON" } else { "OFF" }.to_string(),
        }
    }

    /// Get the placeholder kind
    fn kind(&self) -> PlaceholderKind {
        match self {
            PlaceholderValue::Decimal(_) => PlaceholderKind::Decimal,
            PlaceholderValue::Hex(_) => PlaceholderKind::Hex,
            PlaceholderValue::Boolean(_) => PlaceholderKind::Boolean,
        }
    }
}

#[derive(Clone, Debug)]
struct PlaceholderEntry {
    placeholder: String,
    actual: String,
}

#[derive(Default, Debug)]
struct PlaceholderState {
    counter: usize,
    entries: Vec<PlaceholderEntry>,
}

static PLACEHOLDER_STATE: Lazy<Mutex<PlaceholderState>> =
    Lazy::new(|| Mutex::new(PlaceholderState::default()));

#[derive(Clone, Copy, Debug)]
enum PlaceholderKind {
    Decimal,
    Hex,
    Boolean,
}

impl PlaceholderKind {
    fn build_placeholder(self, index: usize) -> String {
        match self {
            PlaceholderKind::Decimal => format!("{{{{#{:03}}}}}", index),
            PlaceholderKind::Hex => format!("{{{{0x#{:03}}}}}", index),
            PlaceholderKind::Boolean => format!("{{{{0b#{:03}}}}}", index),
        }
    }
}

fn push_entry(kind: PlaceholderKind, actual: String) {
    let mut state = PLACEHOLDER_STATE.lock();
    state.counter += 1;
    let placeholder = kind.build_placeholder(state.counter);
    state.entries.push(PlaceholderEntry {
        placeholder,
        actual,
    });
}

/// Reset the snapshot placeholder registry.
pub fn reset_snapshot_placeholders() {
    let mut state = PLACEHOLDER_STATE.lock();
    state.counter = 0;
    state.entries.clear();
}

/// Register placeholder values that will appear in snapshot output.
pub fn register_placeholder_values(values: &[PlaceholderValue]) {
    for value in values {
        push_entry(value.kind(), value.as_string());
    }
}

/// Register hexadecimal values that will appear in snapshot output.
pub fn register_snapshot_hex_values(values: &[u16]) {
    for &value in values {
        push_entry(PlaceholderKind::Hex, format!("0x{:04X}", value));
    }
}

/// Register switch-style values (ON/OFF) that will appear in snapshot output.
#[deprecated(note = "Use register_placeholder_values with PlaceholderValue::Boolean instead")]
pub fn register_snapshot_switch_values(values: &[u16]) {
    for &value in values {
        let text = if value != 0 { "ON" } else { "OFF" };
        push_entry(PlaceholderKind::Boolean, text.to_string());
    }
}

fn replace_once_from(
    haystack: &mut String,
    needle: &str,
    replacement: &str,
    start: usize,
) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }

    let len = haystack.len();
    let start_idx = start.min(len);

    if let Some(rel_idx) = haystack[start_idx..].find(needle) {
        let idx = start_idx + rel_idx;
        haystack.replace_range(idx..idx + needle.len(), replacement);
        Some(idx + replacement.len())
    } else {
        None
    }
}

/// Apply placeholders to a generated screenshot so random values are hidden in the reference file.
pub(crate) fn apply_placeholders_for_generation(screen: &str) -> String {
    let entries = {
        let state = PLACEHOLDER_STATE.lock();
        state.entries.clone()
    };

    if entries.is_empty() {
        return screen.to_owned();
    }

    let mut result = screen.to_owned();
    let mut search_offset = result.find("  0x").unwrap_or(0);

    for entry in &entries {
        if let Some(next_offset) = replace_once_from(
            &mut result,
            &entry.actual,
            &entry.placeholder,
            search_offset,
        ) {
            search_offset = next_offset;
        } else if let Some(next_offset) =
            replace_once_from(&mut result, &entry.actual, &entry.placeholder, 0)
        {
            search_offset = next_offset;
        } else {
            warn!(
                "snapshot placeholder actual value '{}' not found during generation; keeping literal",
                entry.actual
            );
        }
    }

    result
}

/// Restore placeholders to their actual values prior to verification.
pub(crate) fn restore_placeholders_for_verification(screen: &str) -> String {
    let entries = {
        let state = PLACEHOLDER_STATE.lock();
        state.entries.clone()
    };

    if entries.is_empty() {
        return screen.to_owned();
    }

    let mut result = screen.to_owned();

    for entry in &entries {
        if let Some(idx) = result.find(&entry.placeholder) {
            result.replace_range(idx..idx + entry.placeholder.len(), entry.actual.as_str());
        } else {
            warn!(
                "reference screenshot missing placeholder '{}'; cannot restore value",
                entry.placeholder
            );
        }
    }

    result
}
