use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    i18n::lang,
    protocol::status::{
        read_status,
        types::{
            self,
            port::{PortData, PortState},
        },
        with_port_read,
    },
};

use anyhow::Result;

/// Helper function to derive selection from page state (entry page specific)
pub fn derive_selection_from_page(page: &types::Page, ports_order: &[String]) -> Result<usize> {
    let res = match page {
        types::Page::Entry { cursor } => match cursor {
            Some(types::cursor::EntryCursor::Com { index }) => *index,
            Some(types::cursor::EntryCursor::Refresh) => ports_order.len(),
            Some(types::cursor::EntryCursor::CreateVirtual) => ports_order.len().saturating_add(1),
            Some(types::cursor::EntryCursor::About) => ports_order.len().saturating_add(2),
            None => 0usize,
        },
        types::Page::ModbusDashboard { selected_port, .. }
        | types::Page::ConfigPanel { selected_port, .. }
        | types::Page::LogPanel { selected_port, .. } => *selected_port,
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
                if let Some((pn, st)) =
                    with_port_read(port, |port| (port.port_name.clone(), port.state.clone()))
                {
                    (pn, st)
                } else {
                    log::warn!("Failed to acquire read lock for port {name} while rendering the ports list");
                    (
                        PortData::default().port_name.clone(),
                        PortData::default().state.clone(),
                    )
                }
            } else {
                (default_pd.port_name.clone(), default_pd.state.clone())
            };
            let (state_text, state_style) = match state {
                PortState::Free => (lang().index.port_state_free.clone(), Style::default()),
                PortState::OccupiedByThis { .. } => (
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
            let inner = width.saturating_sub(2);
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
                    use crate::protocol::status::types::ui::InputRawBuffer;
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
        let inner_h = area.height.saturating_sub(2) as usize;
        let used = lines.len();
        let extras_len = extras.len();
        let pad_lines = if inner_h > used + extras_len {
            inner_h - used - extras_len
        } else {
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

        let left_block = Block::default()
            .borders(Borders::ALL)
            .title(Span::raw(format!(" {}", lang().index.com_ports.as_str())));
        let left_para = Paragraph::new(lines).block(left_block);
        frame.render_widget(left_para, area);
        Ok(())
    });

    if res.is_err() {
        let input_block = Block::default()
            .borders(Borders::ALL)
            .title(Span::raw(format!(" {}", lang().index.com_ports.as_str())));
        let left_para = Paragraph::new(Vec::<Line>::new()).block(input_block);
        frame.render_widget(left_para, area);
    }

    Ok(())
}
