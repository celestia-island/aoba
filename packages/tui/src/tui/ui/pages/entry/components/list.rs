use anyhow::Result;

use ratatui::{prelude::*, style::{Color, Modifier, Style}, text::{Line, Span}};
use unicode_width::UnicodeWidthStr;

use crate::{tui::status as types, i18n::lang, tui::{ status::{ port::{PortData, PortState}, read_status, }, ui::{ components::boxed_paragraph::render_boxed_paragraph, pages::entry::SPECIAL_ITEMS_COUNT};

/// Helper function to derive selection from page state (entry page specific)
pub fn derive_selection_from_page(
    page: &crate::tui::status::Page,
    ports_order: &[String],
) -> Result<usize> {
    let res = match page {
        crate::tui::status::Page::Entry { cursor, .. } => match cursor {
            Some(types::cursor::EntryCursor::Com { index }) => *index,
            Some(types::cursor::EntryCursor::Refresh) => ports_order.len(),
            Some(types::cursor::EntryCursor::CreateVirtual) => ports_order.len().saturating_add(1),
            Some(types::cursor::EntryCursor::About) => ports_order.len().saturating_add(2),
            None => 0usize,
        },
        crate::tui::status::Page::ModbusDashboard { selected_port, .. }
        | crate::tui::status::Page::ConfigPanel { selected_port, .. }
        | crate::tui::status::Page::LogPanel { selected_port, .. } => *selected_port,
        _ => 0usize,
    };
    Ok(res)
}

/// Render the left ports list panel
pub fn render_ports_list(frame: &mut Frame, area: Rect, selection: usize) -> Result<()> {
    let res = read_status(|status| {
        let width = area.width as usize;
        let mut lines: Vec<Line> = Vec::new();
        let default_pd = PortData::default();

        for (i, name) in status.ports.order.iter().enumerate() {
            let (name, state) = if let Some(port) = status.ports.map.get(name) {
                (port.port_name.clone(), port.state.clone())
            } else {
                (default_pd.port_name.clone(), default_pd.state.clone())
            };
            let (state_text, state_style) = match state {
                PortState::Free => (lang().index.port_state_free.clone(), Style::default()),
                PortState::OccupiedByThis => (
                    lang().index.port_state_owned.clone(),
                    Style::default().fg(Color::Green),
                ),
                PortState::OccupiedByOther => (
                    lang().index.port_state_other.clone(),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ),
            };

            let prefix = if i == selection { "> " } else { "  " };
            // Account for scrollbar (1 char) when calculating available width
            let inner = width.saturating_sub(3); // Border (2) + Scrollbar (1)
            let name_w = UnicodeWidthStr::width(name.as_str()) + UnicodeWidthStr::width(prefix);
            let state_w = UnicodeWidthStr::width(state_text.as_str());
            let pad = if inner > name_w + state_w {
                inner - name_w - state_w
            } else {
                1
            };
            let spacer = " ".repeat(pad);

            let input_buffer = read_status(|s| Ok(s.temporarily.input_raw_buffer.clone()));
            let mut prefix_style = Style::default();
            if i == selection {
                if let Ok(buf) = input_buffer {
                    use crate::tui::status::ui::InputRawBuffer;
                    if !matches!(buf, InputRawBuffer::None) {
                        prefix_style = Style::default().fg(Color::Yellow);
                    } else {
                        prefix_style = Style::default().fg(Color::Green);
                    }
                } else {
                    prefix_style = Style::default().fg(Color::Green);
                }
            }

            let spans = vec![
                Span::styled(prefix, prefix_style),
                Span::raw(name),
                Span::raw(spacer),
                Span::styled(state_text, state_style),
            ];
            if i == selection {
                let styled = spans
                    .into_iter()
                    .map(|sp| {
                        let mut s = sp.style;
                        s = s.bg(Color::LightGreen);
                        Span::styled(sp.content, s)
                    })
                    .collect::<Vec<_>>();
                lines.push(Line::from(styled));
            } else {
                lines.push(Line::from(spans));
            }
        }

        let extras = vec![
            lang().index.refresh_action.as_str().to_string(),
            lang().index.manual_specify_label.as_str().to_string(),
            lang().index.about_label.as_str().to_string(),
        ];
        // Ensure the number of extras matches the constant
        debug_assert_eq!(
            extras.len(),
            SPECIAL_ITEMS_COUNT,
            "Number of special items must match SPECIAL_ITEMS_COUNT constant"
        );
        let inner_h = area.height.saturating_sub(2) as usize;
        let used = lines.len();
        let extras_len = extras.len();

        // Calculate padding based on the requirements:
        // If ports - 4 doesn't fill the screen, add padding to keep last 3 items at bottom
        // If ports - 4 exceeds screen, add only 1 space before last 3 items
        let total_content = used + extras_len;
        let pad_lines = if total_content.saturating_add(4) < inner_h {
            // Case 1: Not enough content to fill screen minus 4
            // Fill middle with padding and keep last 3 items at bottom
            inner_h - used - extras_len
        } else if used > inner_h.saturating_sub(extras_len).saturating_sub(1) {
            // Case 2: Too much content - add only 1 space between ports and extras
            1
        } else {
            // Normal case: fits comfortably
            0
        };

        for _ in 0..pad_lines {
            lines.push(Line::from(Span::raw("")));
        }

        for (j, lbl) in extras.into_iter().enumerate() {
            let index = status.ports.order.len() + j;
            let prefix = if index == selection { "> " } else { "  " };
            let spans = vec![Span::raw(prefix), Span::raw(lbl)];
            if index == selection {
                let styled = spans
                    .into_iter()
                    .map(|sp| Span::styled(sp.content, Style::default().bg(Color::LightGreen)))
                    .collect::<Vec<_>>();
                lines.push(Line::from(styled));
            } else {
                lines.push(Line::from(spans));
            }
        }

        // Get view_offset from page state
        let view_offset = if let crate::tui::status::Page::Entry { view_offset, .. } = &status.page
        {
            *view_offset
        } else {
            0
        };

        // Calculate viewport height (inner area minus borders)
        let viewport_height = area.height.saturating_sub(2) as usize;

        // Determine if scrollbar should be shown
        let ports_count = status.ports.order.len();
        let show_scrollbar =
            crate::tui::ui::pages::entry::should_show_scrollbar(ports_count, viewport_height);

        Ok((lines, view_offset, show_scrollbar))
    });

    match res {
        Ok((lines, view_offset, show_scrollbar)) => {
            render_boxed_paragraph(
                frame,
                area,
                lines,
                view_offset,
                Some(lang().index.com_ports.as_str()),
                false,
                show_scrollbar,
            );
            Ok(())
        }
        Err(_) => {
            render_boxed_paragraph(
                frame,
                area,
                Vec::<Line>::new(),
                0,
                Some(lang().index.com_ports.as_str()),
                false,
                false,
            );
            Ok(())
        }
    }
}
