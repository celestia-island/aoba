use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

use crate::protocol::status::types::ui::InputRawBuffer;

/// TextState is a small helper enum used by UI components for styling decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextState {
    Normal,
    Selected,
    Chosen,
    Editing,
}

/// Produce spans for a left/right selector rendered as: optionally `< [label] >`.
pub fn selector_spans(base_prefix: &str, label: &str, state: TextState) -> Vec<Span<'static>> {
    let edge_style = match state {
        TextState::Selected => Style::default().fg(Color::Green),
        TextState::Editing | TextState::Chosen => Style::default().fg(Color::Green),
        TextState::Normal => Style::default(),
    };
    let mid_style = match state {
        TextState::Editing => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        TextState::Chosen => Style::default().fg(Color::Green),
        TextState::Selected => Style::default().fg(Color::Green),
        TextState::Normal => Style::default(),
    };

    let mut spans = Vec::new();
    spans.push(Span::raw(base_prefix.to_string()));
    if state == TextState::Editing {
        spans.push(Span::styled("< ", edge_style));
    }
    spans.push(Span::styled(format!("[{label}]"), mid_style));
    if state == TextState::Editing {
        spans.push(Span::styled(" >", edge_style));
    }
    spans
}

/// Produce spans for an input-style display rendered as: `> [buffer] <` with surrounding base_prefix
/// Included as raw text. `hovered` and `editing` affect colors/weight.
pub fn input_spans(
    base_prefix: &str,
    buffer: &str,
    hovered: bool,
    editing: bool,
    with_prefix: bool,
) -> Vec<Span<'static>> {
    let outer_style = if editing {
        Style::default().fg(Color::Yellow)
    } else if hovered {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    let inner_style = if editing {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if hovered {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    let buf = if buffer.is_empty() { "_" } else { buffer };
    let mut out = Vec::new();
    if with_prefix {
        out.push(Span::raw(base_prefix.to_string()));
    }
    // Only show arrows when actually editing (entered second stage)
    if editing {
        out.push(Span::styled("> ", outer_style));
    }
    out.push(Span::styled(format!("[{buf}]"), inner_style));
    if editing {
        out.push(Span::styled(" <", outer_style));
    }
    out
}

/// Helper to produce prefix "> " and index span like "#n" with consistent styling.
pub fn prefix_and_index_spans(idx: usize, selected: bool, chosen: bool) -> Vec<Span<'static>> {
    let normal = Style::default();
    let browse = Style::default().fg(Color::Green);
    let chosen_style = Style::default().fg(Color::Green);

    let prefix_style = if selected {
        if chosen {
            chosen_style
        } else {
            browse
        }
    } else {
        normal
    };

    let mut out: Vec<Span> = Vec::new();
    if selected {
        out.push(Span::styled("> ", prefix_style));
    }
    out.push(Span::styled(format!("#{}", idx + 1), prefix_style));
    out
}

/// Generic helper to produce a styled Span for a text with selection/editing semantics.
/// - `selected` maps to green (hover)
/// - `editing` maps to yellow + bold
/// - `bold` forces bold for non-editing cases
pub fn styled_text(text: &str, state: TextState, bold: bool) -> Span<'static> {
    let mut style = Style::default();
    match state {
        TextState::Editing => {
            style = style.fg(Color::Yellow).add_modifier(Modifier::BOLD);
        }
        TextState::Chosen => {
            style = style.fg(Color::Green);
        }
        TextState::Selected => {
            style = style.fg(Color::Green);
            if bold {
                style = style.add_modifier(Modifier::BOLD);
            }
        }
        TextState::Normal => {
            if bold {
                style = style.add_modifier(Modifier::BOLD);
            }
        }
    }
    Span::styled(text.to_string(), style)
}

/// Generic selector helper: given an iterator of items and a selected index, produce spans
/// using the same visual rules as `selector_spans`. This helper is generic over any
/// iterator of items that can be displayed. It returns the spans and also a helper
/// to convert a selected index into an `InputRawBuffer::Index`.
pub fn generic_selector_spans<T, I>(
    base_prefix: &str,
    items: I,
    selected_idx: Option<usize>,
    editing: bool,
) -> Vec<Span<'static>>
where
    I: IntoIterator<Item = T>,
    T: std::fmt::Display,
{
    // Render items separated by spaces; highlight the selected item
    let mut spans = Vec::new();
    spans.push(Span::raw(base_prefix.to_string()));
    for (i, item) in items.into_iter().enumerate() {
        let is_selected = selected_idx.map_or(false, |s| s == i);
        let state = if editing && is_selected {
            TextState::Editing
        } else if is_selected {
            TextState::Selected
        } else {
            TextState::Normal
        };

        // Only show arrows around the entire selector when editing; individual items
        // use selector_spans semantics but without duplicating base_prefix.
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        // We'll render each item as [item] (no arrows per-item)
        let mid_style = match state {
            TextState::Editing => Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            TextState::Selected => Style::default().fg(Color::Green),
            _ => Style::default(),
        };
        spans.push(Span::styled(format!("[{item}]"), mid_style));
    }

    // If editing, surround the whole thing with arrows
    if editing {
        // Insert opening arrow after base_prefix
        if spans.len() > 0 {
            // base_prefix is at index 0; insert after it
            spans.insert(1, Span::styled("< ", Style::default().fg(Color::Yellow)));
            spans.push(Span::styled(" >", Style::default().fg(Color::Yellow)));
        }
    }

    spans
}

/// Return true if the provided InputRawBuffer selects the given index
pub fn buffer_matches_index(buf: &InputRawBuffer, idx: usize) -> bool {
    match buf {
        InputRawBuffer::Index(i) => *i == idx,
        _ => false,
    }
}

/// Compute a TextState for an indexable widget using the global buffer and an explicit
/// `editing` flag (the caller should pass `true` when the app is in second-stage editing).
pub fn state_for_index(buf: &InputRawBuffer, idx: usize, chosen: bool, editing: bool) -> TextState {
    if buffer_matches_index(buf, idx) {
        if editing {
            TextState::Editing
        } else if chosen {
            TextState::Chosen
        } else {
            TextState::Selected
        }
    } else {
        TextState::Normal
    }
}

/// Produce input spans directly from the global buffer (converts buffer to string internally).
pub fn input_spans_from_buffer(
    base_prefix: &str,
    buf: &InputRawBuffer,
    hovered: bool,
    editing: bool,
    with_prefix: bool,
) -> Vec<Span<'static>> {
    let content = buf.as_string();
    input_spans(base_prefix, content.as_str(), hovered, editing, with_prefix)
}

/// Produce prefix/index spans based on global buffer (selected when index matches buffer Index).
pub fn prefix_and_index_spans_from_buffer(
    idx: usize,
    buf: &InputRawBuffer,
    chosen: bool,
) -> Vec<Span<'static>> {
    let selected = buffer_matches_index(buf, idx);
    prefix_and_index_spans(idx, selected, chosen)
}

/// Generic selector helper that reads the selected index from global buffer when present.
pub fn generic_selector_spans_from_buffer<T, I>(
    base_prefix: &str,
    items: I,
    buf: &InputRawBuffer,
    editing: bool,
) -> Vec<Span<'static>>
where
    I: IntoIterator<Item = T>,
    T: std::fmt::Display,
{
    // Determine selected_idx from the buffer
    let selected_idx = match buf {
        InputRawBuffer::Index(i) => Some(*i),
        _ => None,
    };
    generic_selector_spans(base_prefix, items, selected_idx, editing)
}

/// Small helper to convert an optional selected index into `InputRawBuffer`.
pub fn selected_index_to_input_raw_buffer(idx: Option<usize>) -> InputRawBuffer {
    match idx {
        Some(i) => InputRawBuffer::Index(i),
        None => InputRawBuffer::None,
    }
}

/// Unified kind for producing Span sequences. This lets callers use one helper instead of
/// Multiple similarly-named helpers. Keep variants small and expressive.
pub enum StyledSpanKind<'a> {
    Selector {
        base_prefix: &'a str,
        label: &'a str,
        state: TextState,
    },
    Input {
        base_prefix: &'a str,
        buffer: &'a str,
        hovered: bool,
        editing: bool,
        with_prefix: bool,
    },
    PrefixIndex {
        idx: usize,
        selected: bool,
        chosen: bool,
    },
    Text {
        text: &'a str,
        state: TextState,
        bold: bool,
    },
}

/// Central dispatcher: produce the appropriate spans for a StyledSpanKind.
pub fn styled_spans(kind: StyledSpanKind<'_>) -> Vec<Span<'static>> {
    match kind {
        StyledSpanKind::Selector {
            base_prefix,
            label,
            state,
        } => selector_spans(base_prefix, label, state),
        StyledSpanKind::Input {
            base_prefix,
            buffer,
            hovered,
            editing,
            with_prefix,
        } => input_spans(base_prefix, buffer, hovered, editing, with_prefix),
        StyledSpanKind::PrefixIndex {
            idx,
            selected,
            chosen,
        } => prefix_and_index_spans(idx, selected, chosen),
        StyledSpanKind::Text { text, state, bold } => vec![styled_text(text, state, bold)],
    }
}
