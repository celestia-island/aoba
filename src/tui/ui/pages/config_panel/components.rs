use anyhow::Result;
use std::sync::{Arc, RwLock};
use strum::IntoEnumIterator;

use ratatui::{prelude::*, style::Modifier, text::Line};
use unicode_width::UnicodeWidthStr;

use crate::{
    i18n::lang,
    protocol::status::{read_status, types, with_port_read},
    tui::ui::components::styled_label::{styled_spans, StyledSpanKind, TextState},
};

use crate::protocol::status::types::modbus::ParityOption;

// Constants to avoid magic numbers/strings in layout calculation
const TARGET_LABEL_WIDTH: usize = 20; // target label column width for alignment
const LABEL_PADDING_EXTRA: usize = 2; // extra spacing added after label when padding
const INDICATOR_SELECTED: &str = "> ";
const INDICATOR_UNSELECTED: &str = "  ";

/// Derive selection index for config panel from current page state
pub fn derive_selection() -> Result<types::cursor::ConfigPanelCursor> {
    // For config panel, we need to determine which field is currently selected
    match read_status(|status| Ok(status.page.clone()))? {
        types::Page::ConfigPanel { cursor, .. } => {
            // cursor tracks both navigation and editing state
            Ok(cursor)
        }
        _ => Ok(types::cursor::ConfigPanelCursor::EnablePort),
    }
}

/// Generate lines for config panel with 4:1:5 layout (label:indicator:value).
/// Returns lines that can be used with render_boxed_paragraph.
///
/// The structure follows the requirements:
/// - Group 1: "Enable Port" toggle + "Protocol Mode" selector + "Protocol Config" navigation
/// - Group 2: Serial port basic parameters (baud rate, parity, etc.)
///
/// Each line has the format: [Label____] [>] [Value_____] with proper spacing.
pub fn render_kv_lines_with_indicators(sel_idx: usize) -> Result<Vec<Line<'static>>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Get current port data
    let port_data = if let Some(port_name) =
        read_status(|status| Ok(status.ports.order.get(sel_idx).cloned()))?
    {
        read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))?
    } else {
        None
    };

    // Determine current selection for styling
    let current_selection = derive_selection()?;

    // Determine whether the port is occupied by this instance. Only in that case
    // we display the full set of controls (group2, group3 and protocol config
    // navigation inside group1).
    let occupied_by_this = is_port_occupied_by_this(port_data.as_ref());
    // Build unified item list from static descriptors, items may be filtered
    // depending on whether the port is occupied by this instance.
    let mut items: Vec<ConfigItem> = Vec::new();

    // GROUP 1 items (always include enable & protocol mode; protocol config may be conditional)
    items.push(ConfigItem::enable_port());
    items.push(ConfigItem::protocol_mode());
    if occupied_by_this {
        items.push(ConfigItem::protocol_config());
    }

    // If occupied, add a separator then serial params and log entry
    if occupied_by_this {
        // separator represented by empty label/value
        items.push(ConfigItem::separator());
        items.extend(ConfigItem::serial_params());
        items.push(ConfigItem::separator());
        items.push(ConfigItem::communication_log());
    }

    // Render items with indicators based on current_selection
    for item in items.iter() {
        let selected = current_selection == item.cursor;
        lines.push(create_line(
            &item.label,
            item.clone_for_render(selected, port_data.as_ref()),
            selected,
        )?);
    }

    Ok(lines)
}

/// Helper to determine whether given port_data is occupied by this instance
fn is_port_occupied_by_this(port_data: Option<&Arc<RwLock<types::port::PortData>>>) -> bool {
    if let Some(port) = port_data {
        return with_port_read(port, |port| {
            matches!(&port.state, types::port::PortState::OccupiedByThis { .. })
        })
        .unwrap_or(false);
    }

    false
}

/// Descriptor for a single config item used to drive rendering and input handling.
#[derive(Clone)]
pub struct ConfigItem {
    label: String,
    // We store the StyledSpanKind as the renderer will convert it into spans.
    value_kind: StyledSpanKind,
    cursor: types::cursor::ConfigPanelCursor,
    // action: an optional action that describes what should happen when this
    // item is "activated". Actual side-effects are performed by `input.rs`.
    action: Option<ConfigAction>,
}

impl ConfigItem {
    /// Read-only accessor for the cursor associated with this item.
    pub fn cursor(&self) -> types::cursor::ConfigPanelCursor {
        self.cursor
    }

    /// Read-only accessor for the action associated with this item.
    pub fn action(&self) -> Option<ConfigAction> {
        self.action.clone()
    }
}

/// Build the list of `ConfigItem` descriptors for the given selected port index.
/// This mirrors the items produced by `render_kv_lines_with_indicators`, but
/// returns the descriptors directly so `input.rs` can decide how to apply
/// actions and parse input buffers.
pub fn build_items(sel_idx: usize) -> Result<Vec<ConfigItem>> {
    // Get current port data (cloned Arc)
    let port_data = if let Some(port_name) =
        read_status(|status| Ok(status.ports.order.get(sel_idx).cloned()))?
    {
        read_status(|status| Ok(status.ports.map.get(&port_name).cloned()))?
    } else {
        None
    };

    let occupied_by_this = is_port_occupied_by_this(port_data.as_ref());

    let mut items: Vec<ConfigItem> = Vec::new();
    items.push(ConfigItem::enable_port());
    items.push(ConfigItem::protocol_mode());
    if occupied_by_this {
        items.push(ConfigItem::protocol_config());
    }

    if occupied_by_this {
        items.push(ConfigItem::separator());
        items.extend(ConfigItem::serial_params());
        items.push(ConfigItem::separator());
        items.push(ConfigItem::communication_log());
    }

    Ok(items)
}

/// Action returned by a config item's write handler. The caller (usually input.rs)
/// will interpret these actions and perform UI navigation or send messages to core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigAction {
    None,
    ToggleRuntime,
    GoToModbusPanel,
    GoToLogPanel,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::status::types::cursor::ConfigPanelCursor;

    #[test]
    fn build_items_action_mapping() {
        // Ensure global status is initialized for read_status/write_status helpers
        use crate::protocol::status::{init_status, types::Status};
        use std::sync::{Arc, RwLock};

        let status = Arc::new(RwLock::new(Status::default()));
        // ignore error if already initialized by other tests
        let _ = init_status(status.clone());

        // Build items for port index 0
        let items = build_items(0).expect("build_items failed");

        // Find the item for EnablePort and ensure action is ToggleRuntime or None depending on occupancy.
        let enable_item = items
            .iter()
            .find(|it| it.cursor() == ConfigPanelCursor::EnablePort)
            .expect("EnablePort item missing");

        // The enable item should have an action defined (ToggleRuntime) in the descriptor.
        assert_eq!(enable_item.action(), Some(ConfigAction::ToggleRuntime));

        // ProtocolConfig, if present, should map to GoToModbusPanel when present.
        if let Some(pc) = items
            .iter()
            .find(|it| it.cursor() == ConfigPanelCursor::ProtocolConfig)
        {
            assert_eq!(pc.action(), Some(ConfigAction::GoToModbusPanel));
        }

        // ViewCommunicationLog, if present, should map to GoToLogPanel.
        if let Some(lg) = items
            .iter()
            .find(|it| it.cursor() == ConfigPanelCursor::ViewCommunicationLog)
        {
            assert_eq!(lg.action(), Some(ConfigAction::GoToLogPanel));
        }
    }
}

impl ConfigItem {
    fn separator() -> Self {
        Self {
            label: String::new(),
            value_kind: StyledSpanKind::Text {
                text: String::new(),
                state: TextState::Normal,
                bold: false,
            },
            cursor: types::cursor::ConfigPanelCursor::EnablePort,
            action: None,
        }
    }

    fn enable_port() -> Self {
        let label = lang().protocol.common.enable_port.clone();
        let value = lang().protocol.common.port_disabled.clone();
        Self {
            label,
            value_kind: StyledSpanKind::Selector {
                base_prefix: String::new(),
                items: vec![value],
                selected_idx: None,
                editing: false,
            },
            cursor: types::cursor::ConfigPanelCursor::EnablePort,
            action: Some(ConfigAction::ToggleRuntime),
        }
    }

    fn protocol_mode() -> Self {
        let label = lang().protocol.common.protocol_mode.clone();
        let value = lang().protocol.common.mode_modbus.clone();
        Self {
            label,
            value_kind: StyledSpanKind::Selector {
                base_prefix: String::new(),
                items: vec![value],
                selected_idx: None,
                editing: false,
            },
            cursor: types::cursor::ConfigPanelCursor::ProtocolMode,
            action: Some(ConfigAction::None),
        }
    }

    fn protocol_config() -> Self {
        let label = lang().protocol.common.business_config.clone();
        let value = lang().protocol.common.enter_modbus_config.clone();
        Self {
            label,
            value_kind: StyledSpanKind::Text {
                text: value,
                state: TextState::Normal,
                bold: false,
            },
            cursor: types::cursor::ConfigPanelCursor::ProtocolConfig,
            action: Some(ConfigAction::GoToModbusPanel),
        }
    }

    fn communication_log() -> Self {
        let label = lang().protocol.common.log_monitoring.clone();
        let value = lang().protocol.common.view_communication_log.clone();
        Self {
            label,
            value_kind: StyledSpanKind::Text {
                text: value,
                state: TextState::Normal,
                bold: false,
            },
            cursor: types::cursor::ConfigPanelCursor::ViewCommunicationLog,
            action: Some(ConfigAction::GoToLogPanel),
        }
    }

    fn serial_params() -> Vec<Self> {
        use crate::protocol::status::types::cursor::ConfigPanelCursor;
        vec![
            Self::serial_param(
                ConfigPanelCursor::BaudRate,
                lang().protocol.common.label_baud.clone(),
            ),
            Self::serial_param(
                ConfigPanelCursor::DataBits,
                lang().protocol.common.label_data_bits.clone(),
            ),
            Self::serial_param(
                ConfigPanelCursor::Parity,
                lang().protocol.common.label_parity.clone(),
            ),
            Self::serial_param(
                ConfigPanelCursor::StopBits,
                lang().protocol.common.label_stop_bits.clone(),
            ),
        ]
    }

    fn serial_param(cursor: types::cursor::ConfigPanelCursor, label: String) -> Self {
        // Placeholder value; actual dynamic value comes from get_serial_param_value_by_cursor
        let value = String::new();
        Self {
            label,
            value_kind: StyledSpanKind::Selector {
                base_prefix: String::new(),
                items: vec![value],
                selected_idx: None,
                editing: false,
            },
            cursor,
            action: Some(ConfigAction::None),
        }
    }

    // Note: parity options are constructed directly as i18n strings below.

    /// Return a value-kind adjusted for rendering; for serial params we need to
    /// query the actual port_data value. `selected` determines TextState.
    fn clone_for_render(
        &self,
        selected: bool,
        port_data: Option<&Arc<RwLock<types::port::PortData>>>,
    ) -> StyledSpanKind {
        use {StyledSpanKind, TextState};
        // Determine whether the app is in second-stage editing using the
        // global temporary input buffer. If so, selected items should
        // render in `Editing` state (which causes `selector_spans` to
        // display the surrounding arrows) instead of just `Selected`.
        let global_editing = crate::protocol::status::read_status(|status| {
            Ok(!status.temporarily.input_raw_buffer.is_empty())
        })
        .unwrap_or(false);

        match &self.value_kind {
            StyledSpanKind::Selector {
                base_prefix, items, ..
            } => match self.cursor {
                types::cursor::ConfigPanelCursor::BaudRate
                | types::cursor::ConfigPanelCursor::DataBits
                | types::cursor::ConfigPanelCursor::StopBits => {
                    let val = get_serial_param_value_by_cursor(port_data, self.cursor);
                    StyledSpanKind::Selector {
                        base_prefix: base_prefix.clone(),
                        items: vec![val],
                        selected_idx: if selected { Some(0usize) } else { None },
                        editing: selected && global_editing,
                    }
                }
                types::cursor::ConfigPanelCursor::Parity => {
                    // Build parity options from the ParityOption enum and determine selected index
                    let opts: Vec<String> = ParityOption::iter().map(|p| p.to_string()).collect();

                    // Determine selected index from current runtime config (fallback)
                    let cur_idx = if let Some(port) = port_data {
                        with_port_read(port, |port| {
                            if let types::port::PortState::OccupiedByThis { runtime, .. } =
                                &port.state
                            {
                                match runtime.current_cfg.parity {
                                    serialport::Parity::None => Some(0usize),
                                    serialport::Parity::Odd => Some(1usize),
                                    serialport::Parity::Even => Some(2usize),
                                }
                            } else {
                                Some(0usize)
                            }
                        })
                        .unwrap_or(Some(0usize))
                    } else {
                        Some(0usize)
                    };

                    if selected {
                        // If we're in second-stage editing prefer the global temporary buffer's
                        // Index when present. This allows left/right to modify the buffered
                        // enum index instead of writing directly to runtime until Enter.
                        let buf =
                            read_status(|status| Ok(status.temporarily.input_raw_buffer.clone()))
                                .unwrap_or(types::ui::InputRawBuffer::None);

                        let selected_idx = match buf {
                            types::ui::InputRawBuffer::Index(i) if i < opts.len() => Some(i),
                            _ => cur_idx,
                        };

                        StyledSpanKind::Selector {
                            base_prefix: base_prefix.clone(),
                            items: opts,
                            selected_idx,
                            editing: global_editing,
                        }
                    } else {
                        // Not selected: render only the current value as single-label
                        let cur_label = cur_idx
                            .and_then(|i| opts.get(i).cloned())
                            .unwrap_or_else(|| opts.get(0).cloned().unwrap_or_default());
                        StyledSpanKind::Selector {
                            base_prefix: base_prefix.clone(),
                            items: vec![cur_label],
                            selected_idx: None,
                            editing: false,
                        }
                    }
                }
                types::cursor::ConfigPanelCursor::EnablePort => {
                    let val = if let Some(port) = port_data {
                        if let Some(v) = with_port_read(port, |port| match port.state {
                            types::port::PortState::OccupiedByThis { .. } => {
                                lang().protocol.common.port_enabled.clone()
                            }
                            _ => lang().protocol.common.port_disabled.clone(),
                        }) {
                            v
                        } else {
                            lang().protocol.common.port_disabled.clone()
                        }
                    } else {
                        lang().protocol.common.port_disabled.clone()
                    };
                    StyledSpanKind::Selector {
                        base_prefix: base_prefix.clone(),
                        items: vec![val],
                        selected_idx: if selected { Some(0usize) } else { None },
                        editing: selected && global_editing,
                    }
                }
                types::cursor::ConfigPanelCursor::ProtocolMode => {
                    let val = if let Some(port) = port_data {
                        if let Some(v) = with_port_read(port, |port| match &port.config {
                            types::port::PortConfig::Modbus { .. } => {
                                lang().protocol.common.mode_modbus.clone()
                            }
                        }) {
                            v
                        } else {
                            lang().protocol.common.mode_modbus.clone()
                        }
                    } else {
                        lang().protocol.common.mode_modbus.clone()
                    };
                    StyledSpanKind::Selector {
                        base_prefix: base_prefix.clone(),
                        items: vec![val],
                        selected_idx: if selected { Some(0usize) } else { None },
                        editing: selected && global_editing,
                    }
                }
                _ => StyledSpanKind::Selector {
                    base_prefix: base_prefix.clone(),
                    items: items.clone(),
                    selected_idx: if selected { Some(0usize) } else { None },
                    editing: selected && global_editing,
                },
            },
            StyledSpanKind::Text { text, bold, .. } => StyledSpanKind::Text {
                text: text.clone(),
                state: if selected {
                    TextState::Selected
                } else {
                    TextState::Normal
                },
                bold: *bold,
            },
            // No additional fallback for Selector here; the Selector arm above
            // already handles cursor-specific transformations. If we need a
            // generic passthrough it can be added later, but currently it's
            // unnecessary and caused an unreachable-pattern warning.
            StyledSpanKind::Input {
                base_prefix,
                buffer,
                hovered,
                editing,
                with_prefix,
            } => StyledSpanKind::Input {
                base_prefix: base_prefix.clone(),
                buffer: buffer.clone(),
                hovered: *hovered,
                editing: *editing,
                with_prefix: *with_prefix,
            },
            StyledSpanKind::PrefixIndex {
                idx,
                selected: sel,
                chosen,
            } => StyledSpanKind::PrefixIndex {
                idx: *idx,
                selected: *sel,
                chosen: *chosen,
            },
        }
    }
}

/// Create a config line with dynamic spacing between label and value using unicode-width
fn create_line(
    label: &str,
    // Accept an owned StyledSpanKind so caller provides how the value is rendered.
    value_kind: StyledSpanKind,
    selected: bool,
) -> Result<Line<'static>> {
    // Calculate the width of the label accurately accounting for Unicode
    let label_width = UnicodeWidthStr::width(label);

    // Create spans
    let mut line_spans = Vec::new();

    // Add label if not empty (for hyperlink-style entries, label will be empty)
    if !label.is_empty() {
        let label_span = Span::styled(
            label.to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        );
        line_spans.push(label_span);

        // Calculate dynamic spacing to align values properly
        // Target alignment: labels should take ~40% of width, values start at ~45%
        let padding_needed = if label_width < TARGET_LABEL_WIDTH {
            TARGET_LABEL_WIDTH - label_width + LABEL_PADDING_EXTRA
        } else {
            LABEL_PADDING_EXTRA // Minimum spacing
        };

        // Add spacing
        line_spans.push(Span::raw(" ".repeat(padding_needed)));
    }

    // Add focus indicator
    // For separator-like rows (empty label and empty text value) we don't want
    // to render the focus indicator (it would show a leading '>'). Detect
    // that case and skip the indicator.
    let is_separator = label.is_empty()
        && matches!(&value_kind, StyledSpanKind::Text { text, .. } if text.is_empty());

    if !is_separator {
        let indicator_span = if selected {
            Span::styled(
                INDICATOR_SELECTED.to_string(),
                Style::default().fg(Color::Green),
            )
        } else {
            Span::raw(INDICATOR_UNSELECTED.to_string())
        };
        line_spans.push(indicator_span);
    }

    // If caller provided a Text/Selector kind that references TextState, we may want to
    // override the state according to whether the whole line is selected. To keep
    // caller control while ensuring consistent selection behavior, map certain
    // variants to use the computed TextState when appropriate.
    use {StyledSpanKind, TextState};

    // Preserve the state embedded in the provided `value_kind` for Selector
    // so that earlier logic (e.g. `clone_for_render`) can set `Editing` and
    // have it flow through to the renderer (which will show arrows).
    let value_spans = match value_kind {
        StyledSpanKind::Selector { .. } => styled_spans(value_kind),
        StyledSpanKind::Text { text, bold, .. } => styled_spans(StyledSpanKind::Text {
            text,
            state: if selected {
                TextState::Selected
            } else {
                TextState::Normal
            },
            bold,
        }),
        // For other kinds pass through but attempt to normalize selection state where there
        // is a state field (Input and PrefixIndex don't carry TextState so pass-through).
        other => styled_spans(other),
    };

    // Add value spans to the line
    line_spans.extend(value_spans);

    Ok(Line::from(line_spans))
}

/// Get serial parameter value by cursor type
fn get_serial_param_value_by_cursor(
    port_data: Option<&Arc<RwLock<types::port::PortData>>>,
    cursor_type: types::cursor::ConfigPanelCursor,
) -> String {
    if let Some(port) = port_data {
        if let Some(s) = with_port_read(port, |port| {
            if let types::port::PortState::OccupiedByThis { ref runtime, .. } = &port.state {
                match cursor_type {
                    types::cursor::ConfigPanelCursor::BaudRate => {
                        return runtime.current_cfg.baud.to_string()
                    }
                    types::cursor::ConfigPanelCursor::DataBits => {
                        return runtime.current_cfg.data_bits.to_string()
                    }
                    types::cursor::ConfigPanelCursor::Parity => {
                        return format!("{:?}", runtime.current_cfg.parity)
                    }
                    types::cursor::ConfigPanelCursor::StopBits => {
                        return runtime.current_cfg.stop_bits.to_string()
                    }
                    _ => return "??".to_string(),
                }
            }
            "??".to_string()
        }) {
            return s;
        } else {
            log::warn!("get_serial_param_value_by_cursor: failed to acquire read lock");
        }
    }

    "??".to_string()
}

// NOTE: Input-handling functions (apply_config_edit and invoke_write_handler)
// were intentionally moved to `input.rs` to centralize all input-related logic
// in the same file. The rendering-only module (`components.rs`) should not
// perform input parsing or side-effects. `input.rs` now uses `build_items`
// and interprets `ConfigItem.action` to apply changes.
