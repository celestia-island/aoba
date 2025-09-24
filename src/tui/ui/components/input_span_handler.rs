use anyhow::{anyhow, Result};

use crossterm::event::KeyEvent;

use crate::{
    protocol::status::{read_status, types::ui::InputRawBuffer, write_status},
    tui::utils::bus::Bus,
};

/// Handle keys when we are in an input/span edit mode.
///
/// - `key` is the KeyEvent being handled.
/// - `bus` is the UI/Core message bus to send Refresh/ToggleRuntime etc.
/// - `commit_fn` is a closure that will be called when Enter commits and should
///    receive the final string (for string-mode edits) or None for index-mode commits.
/// Generic input/span handler.
/// - `index_choices`: optional number of choices for Index buffers; when provided,
///   Left/Right will wrap modulo this value. If None, Left will saturate at 0 and
///   Right will increment without wrapping.
// Optional character filter: when present, only characters for which the
// filter returns true will be accepted into string buffers. This is used
// by callers to restrict input to digits, hex digits, etc.
pub fn handle_input_span<F>(
    key: KeyEvent,
    bus: &Bus,
    index_choices: Option<usize>,
    char_filter: Option<Box<dyn Fn(char) -> bool>>,
    mut commit_fn: F,
) -> Result<()>
where
    F: FnMut(Option<String>) -> Result<()>,
{
    // Only operate on the global temporary buffer in a generic way.
    match key.code {
        crossterm::event::KeyCode::Left
        | crossterm::event::KeyCode::Right
        | crossterm::event::KeyCode::Char('h')
        | crossterm::event::KeyCode::Char('l') => {
            let is_right = matches!(
                key.code,
                crossterm::event::KeyCode::Right | crossterm::event::KeyCode::Char('l')
            );

            write_status(|status| {
                match &mut status.temporarily.input_raw_buffer {
                    InputRawBuffer::String { .. } => {
                        let delta: isize = if is_right { 1 } else { -1 };
                        status.temporarily.input_raw_buffer.move_offset(delta);
                    }
                    InputRawBuffer::Index(i) => {
                        if let Some(choices) = index_choices {
                            if choices == 0 {
                                *i = 0;
                            } else if is_right {
                                *i = (*i + 1) % choices;
                            } else {
                                *i = (*i + choices - 1) % choices;
                            }
                        } else {
                            // no wrapping info: saturate at 0 for left, increment for right
                            if is_right {
                                *i = i.saturating_add(1);
                            } else {
                                *i = i.saturating_sub(1);
                            }
                        }
                    }
                    _ => {}
                }
                Ok(())
            })?;

            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        crossterm::event::KeyCode::Char(c) => {
            // Character input: apply optional filter then push into buffer
            let accept = match &char_filter {
                Some(f) => f(c),
                None => true,
            };

            if accept {
                write_status(|status| {
                    status.temporarily.input_raw_buffer.push(c);
                    Ok(())
                })?;

                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|e| anyhow!(e))?;
            }

            Ok(())
        }
        crossterm::event::KeyCode::Enter => {
            // If currently string mode -> send final string; otherwise index mode -> send None to indicate index commit
            let buf = read_status(|status| Ok(status.temporarily.input_raw_buffer.clone()))?;
            match buf {
                InputRawBuffer::String { bytes, .. } => {
                    if let Ok(s) = std::str::from_utf8(&bytes) {
                        let trimmed = s.trim().to_string();
                        commit_fn(Some(trimmed))?;
                    } else {
                        commit_fn(None)?;
                    }
                }
                _ => {
                    commit_fn(None)?;
                }
            }

            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        crossterm::event::KeyCode::Esc => {
            write_status(|status| {
                status.temporarily.input_raw_buffer.clear();
                Ok(())
            })?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        crossterm::event::KeyCode::Backspace | crossterm::event::KeyCode::Delete => {
            write_status(|status| {
                status.temporarily.input_raw_buffer.pop();
                Ok(())
            })?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|e| anyhow!(e))?;
            Ok(())
        }
        _ => Ok(()),
    }
}
