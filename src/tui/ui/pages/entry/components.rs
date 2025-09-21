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
            cursor::EntryCursor,
            port::{PortData, PortState},
        },
        with_port_read,
    },
    tui::ui::{
        components::boxed_paragraph::render_boxed_paragraph,
        pages::about::components::{init_about_cache, render_about_page_manifest_lines},
    },
};

/// Helper function to derive selection from page state (entry page specific)
pub fn derive_selection_from_page(page: &types::Page, ports_order: &[String]) -> usize {
    match page {
        types::Page::Entry { cursor } => match cursor {
            Some(types::cursor::EntryCursor::Com { idx }) => *idx,
            Some(types::cursor::EntryCursor::Refresh) => ports_order.len(),
            Some(types::cursor::EntryCursor::CreateVirtual) => ports_order.len().saturating_add(1),
            Some(types::cursor::EntryCursor::About) => ports_order.len().saturating_add(2),
            None => 0usize,
        },
        types::Page::ModbusDashboard { selected_port, .. }
        | types::Page::ConfigPanel { selected_port, .. }
        | types::Page::LogPanel { selected_port, .. } => *selected_port,
        _ => 0usize,
    }
}

/// Render the left ports list panel
pub fn render_ports_list(frame: &mut Frame, area: Rect, selection: usize) {
    if read_status(|status| {
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
                    log::warn!(
                        "render_ports_list: failed to acquire read lock for {}",
                        name
                    );
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

            let spans = vec![
                Span::raw(prefix),
                Span::raw(name),
                Span::raw(spacer),
                Span::styled(state_text, state_style),
            ];
            if i == selection {
                let styled = spans
                    .into_iter()
                    .map(|sp| Span::styled(sp.content, Style::default().bg(Color::LightGreen)))
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
            let idx = status.ports.order.len() + j;
            let prefix = if idx == selection { "> " } else { "  " };
            let spans = vec![Span::raw(prefix), Span::raw(lbl)];
            if idx == selection {
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
    })
    .is_err()
    {
        let input_block = Block::default()
            .borders(Borders::ALL)
            .title(Span::raw(format!(" {}", lang().index.com_ports.as_str())));
        let left_para = Paragraph::new(Vec::<Line>::new()).block(input_block);
        frame.render_widget(left_para, area);
    }
}

/// Render the right details panel content
pub fn render_details_panel(frame: &mut Frame, area: Rect) {
    // Check if subpage is active first
    if let Ok(subpage_active) =
        read_status(|app| Ok(!matches!(app.page, types::Page::Entry { .. })))
    {
        if subpage_active {
            return;
        }
    }

    // Get content lines based on page state
    if let Ok(content_lines) = read_status(|app| {
        if app.ports.order.is_empty() {
            // No ports available
            Ok(vec![Line::from(lang().index.no_com_ports.as_str())])
        } else if let types::Page::Entry { cursor } = &app.page {
            // Match on cursor to determine content
            match cursor {
                Some(EntryCursor::Com { idx }) => {
                    if *idx < app.ports.order.len() {
                        let lines = crate::tui::ui::pages::config_panel::components::render_kv_lines_with_indicators(*idx)?;
                        Ok(lines)
                    } else {
                        Ok(vec![Line::from(
                            lang().index.invalid_port_selection.as_str(),
                        )])
                    }
                }
                Some(EntryCursor::Refresh) => {
                    let lines = get_refresh_content();
                    Ok(lines)
                }
                Some(EntryCursor::CreateVirtual) => {
                    let lines = get_manual_specify_content();
                    Ok(lines)
                }
                Some(EntryCursor::About) => {
                    let lines = get_about_preview_content();
                    Ok(lines)
                }
                None => {
                    // Default to first port if available
                    if !app.ports.order.is_empty() {
                        let lines = crate::tui::ui::pages::config_panel::components::render_kv_lines_with_indicators(0)?;
                        Ok(lines)
                    } else {
                        Ok(vec![Line::from(lang().index.no_com_ports.as_str())])
                    }
                }
            }
        } else {
            // Fallback for non-entry pages
            Ok(vec![Line::from(lang().index.entry_page_required.as_str())])
        }
    }) {
        render_boxed_paragraph(
            frame,
            area,
            content_lines,
            0,
            Some(&lang().index.details),
            true,
            false,
        );
    } else {
        // Error fallback
        let content_lines = vec![Line::from(lang().index.error_loading_content.as_str())];
        render_boxed_paragraph(frame, area, content_lines, 0, None, false, false);
    }
}

/// Get content lines for refresh entry
fn get_refresh_content() -> Vec<Line<'static>> {
    if let Ok((ts_opt, last_scan_info_clone)) = read_status(|app| {
        let ts = app
            .temporarily
            .scan
            .last_scan_time
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string());
        let info = app.temporarily.scan.last_scan_info.clone();
        Ok((ts, info))
    }) {
        let mut lines: Vec<Line<'static>> = Vec::new();

        if let Some(ts_str) = ts_opt {
            lines.push(Line::from(format!(
                "{} {}",
                lang().index.scan_last_header.as_str(),
                ts_str
            )));
        } else {
            lines.push(Line::from(lang().index.scan_none.as_str()));
        }

        lines.push(Line::from(Span::raw("")));

        if !last_scan_info_clone.is_empty() {
            for l in last_scan_info_clone.lines().take(100) {
                if l.starts_with("ERROR:") {
                    lines.push(Line::from(Span::styled(
                        l.to_string(),
                        Style::default().fg(Color::Red),
                    )));
                } else if !l.trim().is_empty() {
                    lines.push(Line::from(l.to_string()));
                }
            }
            if last_scan_info_clone.len() > 100 {
                lines.push(Line::from(format!(
                    "... ({} {})",
                    last_scan_info_clone.len() - 100,
                    lang().index.scan_truncated_suffix.as_str()
                )));
            }
        }

        lines
    } else {
        vec![Line::from(lang().index.scan_none.as_str())]
    }
}

/// Get content lines for manual specify entry
fn get_manual_specify_content() -> Vec<Line<'static>> {
    vec![Line::from(lang().index.manual_specify_label.as_str())]
}

/// Get content lines for about preview entry
fn get_about_preview_content() -> Vec<Line<'static>> {
    let about_cache = init_about_cache();
    // Limit the lifetime of the MutexGuard by scoping the lock and cloning the data.
    let snapshot = {
        match about_cache.lock() {
            Ok(cache) => cache.clone(),
            Err(_) => return vec![Line::from("About (failed to load content)")],
        }
    };

    render_about_page_manifest_lines(snapshot)
}
