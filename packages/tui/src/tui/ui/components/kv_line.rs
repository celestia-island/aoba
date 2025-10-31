use anyhow::Result;

use ratatui::{prelude::*, style::Modifier, text::Line};
use unicode_width::UnicodeWidthStr;

use crate::tui::ui::components::styled_label::TextState;

// Layout constants (kept consistent with existing callers)
pub(crate) const LABEL_PADDING_EXTRA: usize = 2;
pub(crate) const TARGET_LABEL_WIDTH: usize = 20;
pub(crate) const INDICATOR_SELECTED: &str = "> ";
pub(crate) const INDICATOR_UNSELECTED: &str = "  ";

/// Render a three-column key/value line used across multiple panels.
///
/// - `label`: second column text (will be rendered bold)
/// - `text_state`: the UI text state (Normal/Selected/Editing) used to decide
///   the indicator and its color. Callers may choose the appropriate state
///   based on selection or editing.
/// - `value_renderer`: closure that receives the same `TextState` and returns
///   the value spans (third column) as `Vec<Span<'static>>` wrapped in a
///   `Result` so rendering errors can be propagated.
///
/// The indicator text and style are derived from `text_state` and there is
/// no override parameter anymore.
///
/// Returns a `Line<'static>` suitable for `render_boxed_paragraph`.
pub fn render_kv_line<F>(
    label: impl ToString,
    text_state: TextState,
    value_renderer: F,
) -> Result<Line<'static>>
where
    F: FnOnce(TextState) -> Result<Vec<Span<'static>>>,
{
    let label_str = label.to_string();

    // derive indicator text and style from text state
    let indicator_text = match text_state {
        TextState::Selected | TextState::Editing => INDICATOR_SELECTED.to_string(),
        TextState::Normal => INDICATOR_UNSELECTED.to_string(),
    };

    let indicator_style = match text_state {
        TextState::Editing => Style::default().fg(Color::Yellow),
        TextState::Selected => Style::default().fg(Color::Green),
        TextState::Normal => Style::default(),
    };

    // call value renderer (allows caller to compute dynamic content)
    let value_spans = value_renderer(text_state)?;

    // label width (unicode-aware)
    let label_width = label_str.width();
    let padding_needed = if label_width < TARGET_LABEL_WIDTH {
        TARGET_LABEL_WIDTH - label_width + LABEL_PADDING_EXTRA
    } else {
        LABEL_PADDING_EXTRA
    };

    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::styled(indicator_text, indicator_style));
    spans.push(Span::styled(
        label_str,
        Style::default().add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::raw(" ".repeat(padding_needed)));
    spans.extend(value_spans);

    Ok(Line::from(spans))
}
