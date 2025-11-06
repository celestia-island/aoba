use anyhow::{anyhow, Result};

use crossterm::event::KeyEvent;

use crate::tui::{
    status::{read_status, ui::InputRawBuffer, write_status},
    utils::bus::Bus,
};

/// Handle keys when we are in an input/span edit mode.
///
/// - `key` is the KeyEvent being handled.
/// - `bus` is the UI/Core message bus to send Refresh/ToggleRuntime etc.
/// - `commit_fn` is a closure that will be called when Enter commits and should
///   receive the final string (for string-mode edits) or None for index-mode commits.
///
/// Generic input/span handler.
///
/// - `index_choices`: optional number of choices for Index buffers; when provided,
///   Left/Right will wrap modulo this value. If None, Left will saturate at 0 and
///   Right will increment without wrapping.
// Optional character filter: when present, only characters for which the
// filter returns true will be accepted into string buffers. This is used
// by callers to restrict input to digits, hex digits, etc.
pub fn handle_input_span<F1, F2>(
    key: KeyEvent,
    bus: &Bus,
    index_choices: Option<usize>,
    max_string_len: Option<usize>,
    char_filter_fn: F1,
    mut commit_fn: F2,
) -> Result<()>
where
    F1: FnOnce(char) -> bool,
    F2: FnMut(Option<String>) -> Result<()>,
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
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        crossterm::event::KeyCode::Char(c) => {
            // Character input: apply optional filter then push into buffer
            let mut accept = char_filter_fn(c);

            // If a max_string_len is provided, enforce it for "significant"
            // characters. For hex-limited fields we consider only hex digits
            // as significant; other characters like '0','x','X','+' '-' are
            // allowed but do not count toward the significant-char limit.
            if accept {
                if let Some(max) = max_string_len {
                    // Only enforce when current buffer is a String
                    let buf =
                        read_status(|status| Ok(status.temporarily.input_raw_buffer.clone()))?;
                    if let InputRawBuffer::String { bytes, .. } = buf {
                        if let Ok(s) = std::str::from_utf8(&bytes) {
                            let sig_count = s.chars().filter(|ch| ch.is_ascii_hexdigit()).count();
                            // If incoming char is a hex digit, count it toward the limit
                            if c.is_ascii_hexdigit() && sig_count >= max {
                                accept = false;
                            }
                        }
                    }
                }
            }

            if accept {
                write_status(|status| {
                    status.temporarily.input_raw_buffer.push(c);
                    Ok(())
                })?;

                bus.ui_tx
                    .send(crate::tui::utils::bus::UiToCore::Refresh)
                    .map_err(|err| anyhow!(err))?;
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
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        crossterm::event::KeyCode::Esc => {
            write_status(|status| {
                status.temporarily.input_raw_buffer.clear();
                Ok(())
            })?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        crossterm::event::KeyCode::Backspace | crossterm::event::KeyCode::Delete => {
            write_status(|status| {
                status.temporarily.input_raw_buffer.pop();
                Ok(())
            })?;
            bus.ui_tx
                .send(crate::tui::utils::bus::UiToCore::Refresh)
                .map_err(|err| anyhow!(err))?;
            Ok(())
        }
        _ => Ok(()),
    }
}
