use anyhow::{anyhow, Result};
use strum::IntoEnumIterator;

use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

use crate::tui::status::{read_status, ui::InputRawBuffer};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextState {
    Normal,
    Selected,
    Editing,
}

pub fn selector_spans<'a, T>(current_index: usize, state: TextState) -> Result<Vec<Span<'a>>>
where
    T: IntoEnumIterator + std::fmt::Display + 'a,
{
    Ok(match state {
        TextState::Normal => vec![Span::raw(
            T::iter()
                .nth(current_index)
                .map(|item| item.to_string())
                .ok_or(anyhow!("Index out of bounds"))?,
        )],
        TextState::Selected => vec![Span::styled(
            T::iter()
                .nth(current_index)
                .map(|item| item.to_string())
                .ok_or(anyhow!("Index out of bounds"))?,
            Style::default().fg(Color::Green),
        )],
        TextState::Editing => {
            let selected_index = read_status(|status| {
                Ok(
                    if let InputRawBuffer::Index(index) = status.temporarily.input_raw_buffer {
                        Some(index)
                    } else {
                        None
                    },
                )
            })?
            .filter(|&index| index < T::iter().len());
            let selected_index = selected_index.unwrap_or(current_index);

            vec![
                Span::styled(
                    "< ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    T::iter()
                        .nth(selected_index)
                        .map(|item| item.to_string())
                        .unwrap_or_else(|| {
                            // If the index is out of bounds, wrap to valid range
                            T::iter()
                                .nth(selected_index % T::iter().len())
                                .map(|item| item.to_string())
                                .unwrap_or_else(|| "Invalid".to_string())
                        }),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    " >",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]
        }
    })
}

/// Input spans with placeholder support
/// When current_value is empty and placeholder is provided, displays placeholder in gray italic
pub fn input_spans_with_placeholder<'a>(
    current_value: impl ToString,
    placeholder: Option<impl ToString>,
    state: TextState,
) -> Result<Vec<Span<'a>>> {
    let value_str = current_value.to_string();
    let show_placeholder = value_str.is_empty() && placeholder.is_some();

    let mut out: Vec<Span> = Vec::new();
    match state {
        TextState::Normal => {
            if show_placeholder {
                out.push(Span::styled(
                    placeholder.unwrap().to_string(),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ));
            } else {
                out.push(Span::raw(value_str));
            }
        }
        TextState::Selected => {
            if show_placeholder {
                out.push(Span::styled(
                    placeholder.unwrap().to_string(),
                    Style::default()
                        .fg(Color::Gray)
                        .add_modifier(Modifier::ITALIC),
                ));
            } else {
                out.push(Span::styled(value_str, Style::default().fg(Color::Green)));
            }
        }
        TextState::Editing => {
            // When editing starts, clear placeholder
            let editing_opt = read_status(|status| {
                Ok(
                    if let InputRawBuffer::String { bytes, offset } =
                        &status.temporarily.input_raw_buffer
                    {
                        Some((String::from_utf8_lossy(bytes).to_string(), *offset))
                    } else {
                        None
                    },
                )
            })?;

            let (editing_string, offset) = if let Some((s, o)) = editing_opt {
                (s, o)
            } else {
                let s = if show_placeholder {
                    String::new() // Start with empty string when editing placeholder
                } else {
                    value_str
                };
                let o = s.chars().count() as isize;
                (s, o)
            };

            let chars: Vec<char> = editing_string.chars().collect();
            let len = chars.len() as isize;
            let mut pos_isize = if offset < 0 { len + offset } else { offset };
            if pos_isize < 0 {
                pos_isize = 0;
            }
            if pos_isize > len {
                pos_isize = len;
            }
            let pos = pos_isize as usize;

            let left: String = chars.iter().take(pos).collect();
            let right: String = chars.iter().skip(pos).collect();

            out.push(Span::styled(
                "> ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            out.push(Span::styled(left, Style::default().fg(Color::Yellow)));
            out.push(Span::styled(
                "_",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            out.push(Span::styled(right, Style::default().fg(Color::Yellow)));
            out.push(Span::styled(
                " <",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }
    Ok(out)
}

pub fn input_spans<'a>(current_value: impl ToString, state: TextState) -> Result<Vec<Span<'a>>> {
    input_spans_with_placeholder(current_value, None::<String>, state)
}

pub fn switch_spans<'a>(
    is_selected: bool,
    selected_raw: impl ToString,
    unselected_raw: impl ToString,
    state: TextState,
) -> Result<Vec<Span<'a>>> {
    let mut out: Vec<Span> = Vec::new();

    match state {
        TextState::Normal => {
            out.push(Span::raw(if is_selected {
                selected_raw.to_string()
            } else {
                unselected_raw.to_string()
            }));
        }
        TextState::Selected => {
            out.push(Span::styled(
                if is_selected {
                    selected_raw.to_string()
                } else {
                    unselected_raw.to_string()
                },
                Style::default().fg(Color::Green),
            ));
        }
        TextState::Editing => {
            // Render editing-like visual using yellow color to match input_spans
            out.push(Span::styled(
                if is_selected {
                    selected_raw.to_string()
                } else {
                    unselected_raw.to_string()
                },
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        } //_ => unreachable!(),
    }

    Ok(out)
}

pub fn link_spans<'a>(text: impl ToString, state: TextState) -> Result<Vec<Span<'a>>> {
    let mut out: Vec<Span> = Vec::new();

    match state {
        TextState::Normal => {
            out.push(Span::raw(text.to_string()));
        }
        TextState::Selected => {
            out.push(Span::styled(
                text.to_string(),
                Style::default().fg(Color::Green),
            ));
        }
        TextState::Editing => {
            out.push(Span::styled(
                text.to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }

    Ok(out)
}
