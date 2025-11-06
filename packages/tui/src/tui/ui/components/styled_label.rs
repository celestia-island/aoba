use anyhow::{anyhow, Result};
use strum::IntoEnumIterator;

use ratatui::{style::{Color, Modifier, Style}, text::Span};

use crate::{tui::status::read_status, tui::status::ui::InputRawBuffer};

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

pub fn input_spans<'a>(current_value: impl ToString, state: TextState) -> Result<Vec<Span<'a>>> {
    let mut out: Vec<Span> = Vec::new();
    match state {
        TextState::Normal => {
            out.push(Span::raw(current_value.to_string()));
        }
        TextState::Selected => {
            out.push(Span::styled(
                current_value.to_string(),
                Style::default().fg(Color::Green),
            ));
        }
        TextState::Editing => {
            // Read the temporary input buffer string and offset (if present).
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

            // If there's no editing buffer, fall back to the provided current value
            // and place the cursor at the end.
            let (editing_string, offset) = if let Some((s, o)) = editing_opt {
                (s, o)
            } else {
                let s = current_value.to_string();
                let o = s.chars().count() as isize; // place cursor at end
                (s, o)
            };

            // Split the string by character boundary and compute cursor position.
            let chars: Vec<char> = editing_string.chars().collect();
            let len = chars.len() as isize;
            // Negative offsets count from end: -1 means after last char.
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
            // Left side of cursor
            out.push(Span::styled(left, Style::default().fg(Color::Yellow)));
            // Visible cursor (underscore) at the current offset
            out.push(Span::styled(
                "_",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            // Right side of cursor
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
