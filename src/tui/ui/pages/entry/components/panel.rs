use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    i18n::lang,
    protocol::status::{
        types::{self, cursor::EntryCursor},
    },
    tui::{
        status::read_status,
        ui::{
            components::boxed_paragraph::render_boxed_paragraph,
            pages::about::components::{init_about_cache, render_about_page_manifest_lines},
        },
    },
};

use anyhow::Result;

/// Render the right details panel content
pub fn render_details_panel(frame: &mut Frame, area: Rect) -> Result<()> {
    if let Ok(content_lines) = read_status(|app| {
        if app.ports.order.is_empty() {
            Ok(vec![Line::from(lang().index.no_com_ports.as_str())])
        } else if let crate::tui::status::Page::Entry { cursor, .. } = &app.page {
            match cursor {
                Some(EntryCursor::Com { index }) => {
                    if *index < app.ports.order.len() {
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
                    if !app.ports.order.is_empty() {
                        let lines = render_port_basic_info_lines(0);
                        Ok(lines)
                    } else {
                        Ok(vec![Line::from(lang().index.no_com_ports.as_str())])
                    }
                }
            }
        } else {
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
        let content_lines = vec![Line::from(lang().index.error_loading_content.as_str())];
        render_boxed_paragraph(frame, area, content_lines, 0, None, false, false);
    }

    Ok(())
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
    let port_arc_opt: Option<std::sync::Arc<parking_lot::RwLock<types::port::PortData>>> =
        read_status(|s| {
            Ok(s.ports
                .order
                .get(index)
                .and_then(|name| s.ports.map.get(name).cloned()))
        })
        .unwrap_or(None);

    if let Some(port) = port_arc_opt {
        let pn = port.port_name.clone();
        let st = port.state.clone();
        let cfg = match &port.config {
            types::port::PortConfig::Modbus { .. } => {
                // TUI no longer uses runtime handles directly
                // Config would come from subprocess info if needed
                None
            }
        };
        
        let mut lines: Vec<Line<'static>> = Vec::new();

        let enabled = matches!(st, types::port::PortState::OccupiedByThis);
        let val_enabled = lang().protocol.common.port_enabled.clone();
        let val_disabled = lang().protocol.common.port_disabled.clone();
        let enable_text = if enabled { val_enabled } else { val_disabled };
        lines.push(Line::from(Span::raw(format!("{} {}", "", enable_text))));

        if let Some(cfg) = cfg {
            let sep_len = 48usize;
                let sep_str: String = "─".repeat(sep_len);
                lines.push(Line::from(Span::styled(
                    sep_str,
                    Style::default().fg(Color::DarkGray),
                )));

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
                    let lab_w = UnicodeWidthStr::width(label.as_str());
                    let pad = if left_min_width > lab_w {
                        left_min_width - lab_w
                    } else {
                        1
                    };
                    let spacer = " ".repeat(pad);

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

    match render_about_page_manifest_lines(snapshot) {
        Ok(v) => v,
        Err(_) => vec![Line::from("About (failed to render)")],
    }
}
