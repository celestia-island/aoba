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
                    log::warn!("render_ports_list: failed to acquire read lock for {name}");
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
                Some(EntryCursor::Com { index }) => {
                    if *index < app.ports.order.len() {
                        // Use a local, simpler renderer for port basic info to avoid
                        // cursor/selection coupling bugs from the full config panel.
                        let lines = render_port_basic_info_lines(*index);
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
                        let lines = render_port_basic_info_lines(0);
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

/// Render a simplified, local-only port basic info block for the entry page.
/// This intentionally does not depend on the full ConfigPanel renderer to
/// avoid cursor/selection coupling issues. It shows enabled state and common
/// serial parameters in a compact layout.
fn render_port_basic_info_lines(index: usize) -> Vec<Line<'static>> {
    // Fetch the port Arc once under the status read lock to minimize lock churn
    let port_arc_opt: Option<std::sync::Arc<std::sync::RwLock<types::port::PortData>>> =
        match read_status(|s| {
            Ok(s.ports
                .order
                .get(index)
                .and_then(|name| s.ports.map.get(name).cloned()))
        }) {
            Ok(v) => v,
            Err(_) => None,
        };

    if let Some(port_arc) = port_arc_opt {
        // Read port data safely
        if let Some((_pn, state, cfg_opt)) = with_port_read(&port_arc, |port| {
            let pn = port.port_name.clone();
            let st = port.state.clone();
            let cfg = match &port.config {
                types::port::PortConfig::Modbus { .. } => {
                    if let types::port::PortState::OccupiedByThis { runtime, .. } = &port.state {
                        Some(runtime.current_cfg.clone())
                    } else {
                        None
                    }
                }
            };
            (pn, st, cfg)
        }) {
            let mut lines: Vec<Line<'static>> = Vec::new();

            // Enabled switch
            let enabled = matches!(state, types::port::PortState::OccupiedByThis { .. });
            let val_enabled = lang().protocol.common.port_enabled.clone();
            let val_disabled = lang().protocol.common.port_disabled.clone();
            let enable_text = if enabled { val_enabled } else { val_disabled };
            lines.push(Line::from(Span::raw(format!("{} {}", "", enable_text))));

            // Serial params: Baud, DataBits, Parity, StopBits
            if let Some(cfg) = cfg_opt {
                // Separator line
                let sep_len = 48usize;
                let sep_str: String = std::iter::repeat('─').take(sep_len).collect();
                lines.push(Line::from(Span::styled(
                    sep_str,
                    Style::default().fg(Color::DarkGray),
                )));

                // Align labels and values into two columns (label | value).
                // Define a minimum left column width and pad labels using
                // unicode-width so CJK and other wide chars are counted correctly.
                let left_min_width: usize = 14; // reasonable default to match config panel style

                let kv_pairs = vec![
                    (
                        lang().protocol.common.label_baud.as_str().to_string(),
                        cfg.baud.to_string(),
                    ),
                    (
                        lang().protocol.common.label_data_bits.as_str().to_string(),
                        cfg.data_bits.to_string(),
                    ),
                    (
                        lang().protocol.common.label_parity.as_str().to_string(),
                        format!("{:?}", cfg.parity),
                    ),
                    (
                        lang().protocol.common.label_stop_bits.as_str().to_string(),
                        cfg.stop_bits.to_string(),
                    ),
                ];

                for (label, val) in kv_pairs {
                    // compute display width of label and padding needed
                    let lab_w = UnicodeWidthStr::width(label.as_str());
                    let pad = if left_min_width > lab_w {
                        left_min_width - lab_w
                    } else {
                        1
                    };
                    let spacer = " ".repeat(pad);

                    // make label bold and value normal; use Span for styling
                    let spans = vec![
                        Span::styled(label, Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(spacer),
                        Span::raw(val),
                    ];
                    lines.push(Line::from(spans));
                }
            }

            return lines;
        }
    }

    // Fallback when port info is absent
    vec![Line::from(lang().index.invalid_port_selection.as_str())]
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
