pub mod config_panel;
pub mod log_input;
pub mod log_panel;
pub mod modbus_panel;
pub mod mode_selector;
pub mod mqtt_panel;

use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use unicode_width::UnicodeWidthStr;

/// Explicit text state for helper styling to avoid boolean parameter ambiguity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextState {
    Normal,
    Selected,
    Chosen,
    Editing,
}

/// Produce a title span (bold) for a label. When `selected` is true the title is green; when
/// `editing` is true the title is yellow. Always bold to match existing UI conventions.
pub fn styled_title_span(label: &str, selected: bool, editing: bool) -> Span<'static> {
    let title_style = if selected && !editing {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else if selected && editing {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::BOLD)
    };
    Span::styled(label.to_string(), title_style)
}

/// Produce spans for a left/right selector rendered as: `< [label] >`.
/// When `hovered` use green; when `editing` use yellow+bold.
pub fn selector_spans(base_prefix: &str, label: &str, state: TextState) -> Vec<Span<'static>> {
    // Always render selector as left/right arrows: `< [label] >`.
    // Color mapping:
    // - Normal: default (no color)
    // - Selected: green (hover)
    // - Chosen / Editing: yellow (active)
    // Editing additionally is bold for the middle label.
    let edge_style = match state {
        TextState::Selected => Style::default().fg(Color::Green),
        TextState::Editing | TextState::Chosen => Style::default().fg(Color::Yellow),
        TextState::Normal => Style::default(),
    };
    let mid_style = match state {
        TextState::Editing => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        TextState::Chosen => Style::default().fg(Color::Yellow),
        TextState::Selected => Style::default().fg(Color::Green),
        TextState::Normal => Style::default(),
    };
    vec![
        Span::raw(base_prefix.to_string()),
        Span::styled("< ", edge_style),
        Span::styled(format!("[{label}]"), mid_style),
        Span::styled(" >", edge_style),
    ]
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
    // Outer arrows should be yellow when editing, green when hovered, otherwise default.
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
    out.push(Span::styled("> ", outer_style));
    out.push(Span::styled(format!("[{buf}]"), inner_style));
    out.push(Span::styled(" <", outer_style));
    out
}

/// Helper to produce prefix "> " and index span like "#n" with consistent styling.
pub fn prefix_and_index_spans(idx: usize, selected: bool, chosen: bool) -> Vec<Span<'static>> {
    let normal = Style::default();
    let browse = Style::default().fg(Color::Green);
    let chosen_style = Style::default().fg(Color::Yellow);
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
    let mut s = Style::default();
    match state {
        TextState::Editing => {
            s = s.fg(Color::Yellow).add_modifier(Modifier::BOLD);
        }
        TextState::Chosen => {
            s = s.fg(Color::Yellow);
        }
        TextState::Selected => {
            s = s.fg(Color::Green);
            if bold {
                s = s.add_modifier(Modifier::BOLD);
            }
        }
        TextState::Normal => {
            if bold {
                s = s.add_modifier(Modifier::BOLD);
            }
        }
    }
    Span::styled(text.to_string(), s)
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

/// Render a boxed paragraph. Accepts a list of lines, a target rect, and an optional style for the
/// Paragraph content. The block will use all borders by default.
pub fn render_boxed_paragraph(
    f: &mut Frame,
    area: Rect,
    lines: Vec<ratatui::text::Line>,
    style: Option<Style>,
) {
    let block = Block::default().borders(ratatui::widgets::Borders::ALL);
    let mut para = Paragraph::new(lines)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: true });
    if let Some(s) = style {
        para = para.style(s);
    }
    f.render_widget(para, area);
}

/// Convert label/value pairs into aligned `Line`s. Each pair is (label, value, optional style)
/// `indent` is prefixed before each label (for example two spaces). `gap` is the number of
/// spaces between the label column and the value column.
pub fn kv_pairs_to_lines(
    pairs: &[(String, String, Option<Style>)],
    indent: &str,
    gap: usize,
) -> Vec<Line<'static>> {
    // compute max label width
    let max_label_w = pairs
        .iter()
        .map(|(lbl, _, _)| UnicodeWidthStr::width(lbl.as_str()))
        .max()
        .unwrap_or(0usize);
    let mut out: Vec<Line> = Vec::new();
    for (lbl, val, maybe_style) in pairs.iter() {
        let lbl_w = UnicodeWidthStr::width(lbl.as_str());
        let fill = max_label_w.saturating_sub(lbl_w);
        let padded_label = format!("{}{}{}", indent, lbl, " ".repeat(fill));
        let spacer = " ".repeat(gap);
        let label_span = Span::styled(padded_label, Style::default().add_modifier(Modifier::BOLD));
        match maybe_style {
            Some(s) => out.push(Line::from(vec![
                label_span,
                Span::raw(spacer),
                Span::styled(val.clone(), *s),
            ])),
            None => out.push(Line::from(vec![
                label_span,
                Span::raw(spacer),
                Span::raw(val.clone()),
            ])),
        }
    }
    out
}
