use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

use strum::IntoEnumIterator;

use crate::protocol::status::types;

/// TextState is a small helper enum used by UI components for styling decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextState {
    Normal,
    Selected,
    Chosen,
    Editing,
}

/// Produce spans for a left/right selector driven by an enum type `E` that implements
/// `IntoEnumIterator`. The `index` parameter selects which enum variant is active.
/// `state` controls whether the selector is in editing mode (arrows visible) or not.
pub fn selector_spans<T>(base_prefix: &str, index: usize, state: TextState) -> Vec<Span<'static>>
where
    T: IntoEnumIterator + std::fmt::Display,
{
    // Build display labels from the enum variants
    let items: Vec<String> = T::iter().map(|v| v.to_string()).collect();
    let selected_idx = if index < items.len() {
        Some(index)
    } else {
        None
    };
    let editing = state == TextState::Editing;

    generic_selector_spans(base_prefix, items, selected_idx, editing)
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
/// to convert a selected index into an `types::ui::InputRawBuffer::Index`.
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
        let is_selected = selected_idx == Some(i);
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
        if !spans.is_empty() {
            // base_prefix is at index 0; insert after it
            spans.insert(1, Span::styled("< ", Style::default().fg(Color::Yellow)));
            spans.push(Span::styled(" >", Style::default().fg(Color::Yellow)));
        }
    }

    spans
}

/// Return true if the provided types::ui::InputRawBuffer selects the given index
pub fn buffer_matches_index(buf: &types::ui::InputRawBuffer, idx: usize) -> bool {
    match buf {
        types::ui::InputRawBuffer::Index(i) => *i == idx,
        _ => false,
    }
}

/// Compute a TextState for an indexable widget using the global buffer and an explicit
/// `editing` flag (the caller should pass `true` when the app is in second-stage editing).
pub fn state_for_index(
    buf: &types::ui::InputRawBuffer,
    idx: usize,
    chosen: bool,
    editing: bool,
) -> TextState {
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
    buf: &types::ui::InputRawBuffer,
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
    buf: &types::ui::InputRawBuffer,
    chosen: bool,
) -> Vec<Span<'static>> {
    let selected = buffer_matches_index(buf, idx);
    prefix_and_index_spans(idx, selected, chosen)
}

/// Generic selector helper that reads the selected index from global buffer when present.
pub fn generic_selector_spans_from_buffer<T, I>(
    base_prefix: &str,
    items: I,
    buf: &types::ui::InputRawBuffer,
    editing: bool,
) -> Vec<Span<'static>>
where
    I: IntoIterator<Item = T>,
    T: std::fmt::Display,
{
    // Determine selected_idx from the buffer
    let selected_idx = match buf {
        types::ui::InputRawBuffer::Index(i) => Some(*i),
        _ => None,
    };
    generic_selector_spans(base_prefix, items, selected_idx, editing)
}

/// Small helper to convert an optional selected index into `types::ui::InputRawBuffer`.
pub fn selected_index_to_input_raw_buffer(idx: Option<usize>) -> types::ui::InputRawBuffer {
    match idx {
        Some(i) => types::ui::InputRawBuffer::Index(i),
        None => types::ui::InputRawBuffer::None,
    }
}

/// Unified kind for producing Span sequences. This lets callers use one helper instead of
/// Multiple similarly-named helpers. Keep variants small and expressive.
#[derive(Debug, Clone, PartialEq)]
pub enum StyledSpanKind {
    Selector {
        base_prefix: String,
        items: Vec<String>,
        selected_idx: Option<usize>,
        editing: bool,
    },
    Input {
        base_prefix: String,
        buffer: String,
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
        text: String,
        state: TextState,
        bold: bool,
    },
}

/// Central dispatcher: produce the appropriate spans for a StyledSpanKind.
pub fn styled_spans(kind: StyledSpanKind) -> Vec<Span<'static>> {
    match kind {
        StyledSpanKind::Selector {
            base_prefix,
            items,
            selected_idx,
            editing,
        } => generic_selector_spans(base_prefix.as_str(), items, selected_idx, editing),
        StyledSpanKind::Input {
            base_prefix,
            buffer,
            hovered,
            editing,
            with_prefix,
        } => input_spans(
            base_prefix.as_str(),
            buffer.as_str(),
            hovered,
            editing,
            with_prefix,
        ),
        StyledSpanKind::PrefixIndex {
            idx,
            selected,
            chosen,
        } => prefix_and_index_spans(idx, selected, chosen),
        StyledSpanKind::Text { text, state, bold } => vec![styled_text(text.as_str(), state, bold)],
    }
}
