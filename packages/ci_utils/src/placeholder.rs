use log::warn;
use once_cell::sync::Lazy;
use parking_lot::Mutex;

/// Placeholder value type for screenshot generation
/// Each variant holds the actual value needed to locate it in the screenshot
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceholderValue {
    /// Decimal number (e.g., 123 -> {{#xxx}})
    Dec(u16),
    /// Hexadecimal number (e.g., 0x1234 -> {{0x#xxx}})
    Hex(u16),
    /// Boolean value (e.g., ON/OFF -> {{0b#xxx}})
    /// For Boolean, we scan for "OFF" sequentially, no need to match actual value
    Boolean(bool),
}

impl PlaceholderValue {
    /// Get the actual value as a string
    pub fn as_string(&self) -> String {
        match self {
            PlaceholderValue::Dec(v) => format!("{}", v),
            PlaceholderValue::Hex(v) => format!("0x{:04X}", v),
            PlaceholderValue::Boolean(b) => if *b { "ON" } else { "OFF" }.to_string(),
        }
    }

    /// Get the placeholder kind
    fn kind(&self) -> PlaceholderKind {
        match self {
            PlaceholderValue::Dec(_) => PlaceholderKind::Dec,
            PlaceholderValue::Hex(_) => PlaceholderKind::Hex,
            PlaceholderValue::Boolean(_) => PlaceholderKind::Boolean,
        }
    }
}

#[derive(Clone, Debug)]
struct PlaceholderEntry {
    index: usize,
    kind: PlaceholderKind,
    actual: String,
}

#[derive(Debug)]
struct PlaceholderState {
    entries: Vec<PlaceholderEntry>,
    next_index: usize,
}

impl Default for PlaceholderState {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            next_index: 0,
        }
    }
}

static PLACEHOLDER_STATE: Lazy<Mutex<PlaceholderState>> =
    Lazy::new(|| Mutex::new(PlaceholderState::default()));

#[derive(Clone, Copy, Debug)]
enum PlaceholderKind {
    Dec,
    Hex,
    Boolean,
}

impl PlaceholderKind {
    /// Build placeholder for the given index
    /// Each kind has its own format
    fn build_placeholder(self, index: usize) -> String {
        match self {
            PlaceholderKind::Dec => format!("{{{{#{:03}}}}}", index),
            PlaceholderKind::Hex => format!("{{{{0x#{:03}}}}}", index),
            PlaceholderKind::Boolean => format!("{{{{0b#{:03}}}}}", index),
        }
    }
}

/// Reset the snapshot placeholder registry.
pub fn reset_snapshot_placeholders() {
    let mut state = PLACEHOLDER_STATE.lock();
    state.entries.clear();
    state.next_index = 0;
}

/// Register placeholder values that will appear in snapshot output.
/// Index is based on order in the array (0, 1, 2, ...)
pub fn register_placeholder_values(values: &[PlaceholderValue]) {
    let mut state = PLACEHOLDER_STATE.lock();
    for value in values {
        let idx = state.next_index;
        state.next_index += 1;
        state.entries.push(PlaceholderEntry {
            index: idx,
            kind: value.kind(),
            actual: value.as_string(),
        });
    }
}

/// Register hexadecimal values that will appear in snapshot output.
pub fn register_snapshot_hex_values(values: &[u16]) {
    let mut state = PLACEHOLDER_STATE.lock();
    for &value in values {
        let idx = state.next_index;
        state.next_index += 1;
        state.entries.push(PlaceholderEntry {
            index: idx,
            kind: PlaceholderKind::Hex,
            actual: format!("0x{:04X}", value),
        });
    }
}

/// Register switch-style values (ON/OFF) that will appear in snapshot output.
#[deprecated(note = "Use register_placeholder_values with PlaceholderValue::Boolean instead")]
pub fn register_snapshot_switch_values(values: &[u16]) {
    let mut state = PLACEHOLDER_STATE.lock();
    for &value in values {
        let idx = state.next_index;
        state.next_index += 1;
        let text = if value != 0 { "ON" } else { "OFF" };
        state.entries.push(PlaceholderEntry {
            index: idx,
            kind: PlaceholderKind::Boolean,
            actual: text.to_string(),
        });
    }
}

/// Apply placeholders to a generated screenshot so random values are hidden in the reference file.
/// For Boolean placeholders: Scans for "OFF" (or "ON") and replaces sequentially by index
/// For Dec/Hex placeholders: Replaces actual values with their respective placeholder formats
pub(crate) fn apply_placeholders_for_generation(screen: &str) -> String {
    let entries = {
        let state = PLACEHOLDER_STATE.lock();
        state.entries.clone()
    };

    if entries.is_empty() {
        return screen.to_owned();
    }

    let mut result = screen.to_owned();

    // Track scan positions per kind to ensure sequential replacement
    let mut bool_search_offset = 0usize;

    for entry in &entries {
        let placeholder = entry.kind.build_placeholder(entry.index);

        match entry.kind {
            PlaceholderKind::Boolean => {
                // For Boolean: sequentially scan for OFF/ON occurrences without random data
                if let Some(relative_pos) = result[bool_search_offset..].find(&entry.actual) {
                    let absolute_pos = bool_search_offset + relative_pos;
                    result.replace_range(
                        absolute_pos..absolute_pos + entry.actual.len(),
                        &placeholder,
                    );
                    bool_search_offset = absolute_pos + placeholder.len();
                } else {
                    warn!(
                        "Boolean placeholder target '{}' not found for index {} during generation",
                        entry.actual, entry.index
                    );
                }
            }
            PlaceholderKind::Dec | PlaceholderKind::Hex => {
                // For numeric: Replace the first occurrence after register grid area
                let search_start = result.find("  0x").unwrap_or(0);
                if let Some(pos) = result[search_start..].find(&entry.actual) {
                    let absolute_pos = search_start + pos;
                    result.replace_range(
                        absolute_pos..absolute_pos + entry.actual.len(),
                        &placeholder,
                    );
                } else if let Some(pos) = result.find(&entry.actual) {
                    // Fallback: search from beginning
                    result.replace_range(pos..pos + entry.actual.len(), &placeholder);
                } else {
                    warn!(
                        "Numeric placeholder value '{}' not found for index {} during generation",
                        entry.actual, entry.index
                    );
                }
            }
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
        let placeholder = entry.kind.build_placeholder(entry.index);

        if let Some(idx) = result.find(&placeholder) {
            result.replace_range(idx..idx + placeholder.len(), &entry.actual);
        } else {
            warn!(
                "Reference screenshot missing placeholder for index {}; cannot restore value",
                entry.index
            );
        }
    }

    result
}
