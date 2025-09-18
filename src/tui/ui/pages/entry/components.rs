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
            ui::EntryCursor,
        },
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
            Some(types::ui::EntryCursor::Com { idx }) => *idx,
            Some(types::ui::EntryCursor::Refresh) => ports_order.len(),
            Some(types::ui::EntryCursor::CreateVirtual) => ports_order.len().saturating_add(1),
            Some(types::ui::EntryCursor::About) => ports_order.len().saturating_add(2),
            None => 0usize,
        },
        types::Page::ModbusDashboard { selected_port, .. }
        | types::Page::ModbusConfig { selected_port, .. }
        | types::Page::ModbusLog { selected_port, .. } => *selected_port,
        _ => 0usize,
    }
}

/// Render the left ports list panel
pub fn render_ports_list(frame: &mut Frame, area: Rect, selection: usize) {
    if read_status(|s| {
        let width = area.width as usize;
        let mut lines: Vec<Line> = Vec::new();
        let default_pd = PortData::default();

        for (i, name) in s.ports.order.iter().enumerate() {
            let p = s.ports.map.get(name).unwrap_or(&default_pd);
            let name = p.port_name.clone();
            let state = p.state.clone();
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
            let idx = s.ports.order.len() + j;
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
pub fn render_details_panel(frame: &mut Frame, area: Rect, selection: usize) {
    if let Ok(()) = read_status(|app| {
        let subpage_active = matches!(
            app.page,
            types::Page::ModbusConfig { .. }
                | types::Page::ModbusDashboard { .. }
                | types::Page::ModbusLog { .. }
                | types::Page::About { .. }
        );
        if subpage_active {
            return Ok(());
        }

        let content_block = Block::default()
            .borders(Borders::ALL)
            .title(Span::raw(format!(" {}", lang().index.details.as_str())));

        if app.ports.order.is_empty() {
            let content_lines = vec![Line::from(lang().index.no_com_ports.as_str())];
            render_boxed_paragraph(frame, area, content_lines, 0, None, Some(content_block), false);
        } else {
            // Use cursor to determine what to render
            if let types::Page::Entry { cursor } = &app.page {
                match cursor {
                    Some(EntryCursor::Com { idx }) => {
                        if *idx < app.ports.order.len() {
                            let port_name = &app.ports.order[*idx];
                            let port_data = app.ports.map.get(port_name);
                            render_port_details(frame, area, *idx, port_data, content_block);
                        } else {
                            let content_lines = vec![Line::from("Invalid port selection")];
                            render_boxed_paragraph(frame, area, content_lines, 0, None, Some(content_block), false);
                        }
                    }
                    Some(EntryCursor::Refresh) => {
                        render_refresh_content(frame, area, content_block, &app.temporarily);
                    }
                    Some(EntryCursor::CreateVirtual) => {
                        render_manual_specify_content(frame, area, content_block);
                    }
                    Some(EntryCursor::About) => {
                        render_about_preview_content(frame, area);
                    }
                    None => {
                        // Default to first port if available
                        if !app.ports.order.is_empty() {
                            let port_name = &app.ports.order[0];
                            let port_data = app.ports.map.get(port_name);
                            render_port_details(frame, area, 0, port_data, content_block);
                        } else {
                            let content_lines = vec![Line::from(lang().index.no_com_ports.as_str())];
                            render_boxed_paragraph(frame, area, content_lines, 0, None, Some(content_block), false);
                        }
                    }
                }
            } else {
                // Fallback for non-entry pages
                let content_lines = vec![Line::from("Entry page required")];
                render_boxed_paragraph(frame, area, content_lines, 0, None, Some(content_block), false);
            }
        }
        Ok(())
    }) {
    } else {
        let content_block = Block::default()
            .borders(Borders::ALL)
            .title(Span::raw(format!(" {}", lang().index.details.as_str())));
        let content_lines = vec![Line::from(lang().index.no_com_ports.as_str())];
        render_boxed_paragraph(frame, area, content_lines, 0, None, Some(content_block), false);
    }
}

/// Render content for refresh special entry
fn render_refresh_content(f: &mut Frame, area: Rect, content_block: Block, temp_status: &types::TempStatus) {
    let mut lines: Vec<Line> = Vec::new();
    
    // First line: last refresh time (no title)
    if let Some(ts) = temp_status.scan.last_scan_time {
        lines.push(Line::from(format!(
            "{} {}",
            lang().index.scan_last_header.as_str(),
            ts.format("%Y-%m-%d %H:%M:%S")
        )));
    } else {
        lines.push(Line::from(lang().index.scan_none.as_str()));
    }
    
    // Empty line separator
    lines.push(Line::from(""));
    
    // Raw port information - only show what exists, don't show "none" for missing fields
    if !temp_status.scan.last_scan_info.is_empty() {
        for l in temp_status.scan.last_scan_info.lines().take(100) {
            if l.starts_with("ERROR:") {
                lines.push(Line::from(Span::styled(l, Style::default().fg(Color::Red))));
            } else if !l.trim().is_empty() {
                // Only add non-empty lines
                lines.push(Line::from(l));
            }
        }
        if temp_status.scan.last_scan_info.len() > 100 {
            lines.push(Line::from(format!(
                "... ({} {})",
                temp_status.scan.last_scan_info.len() - 100,
                lang().index.scan_truncated_suffix.as_str()
            )));
        }
    }
    
    render_boxed_paragraph(f, area, lines, 0, Some(content_block), false);
}

/// Render content for manual specify special entry
fn render_manual_specify_content(f: &mut Frame, area: Rect, content_block: Block) {
    let content_lines = vec![Line::from(lang().index.manual_specify_label.as_str())];
    render_boxed_paragraph(f, area, content_lines, 0, Some(content_block), false);
}

/// Render content for about special entry (preview)
fn render_about_preview_content(f: &mut Frame, area: Rect) {
    let about_cache = init_about_cache();
    if let Ok(cache) = about_cache.lock() {
        let content_lines = render_about_page_manifest_lines(cache.clone());
        // Use render_boxed_paragraph with title as requested
        render_boxed_paragraph(f, area, content_lines, 0, Some(lang().index.about_label.as_str()), None, false);
    } else {
        let content_lines = vec![Line::from("About (failed to load content)")];
        render_boxed_paragraph(f, area, content_lines, 0, Some(lang().index.about_label.as_str()), None, false);
    }
}

/// Render detailed information for a specific port
/// Render enhanced detailed information for a specific port
fn render_port_details(
    f: &mut Frame,
    area: Rect,
    port_index: usize,
    port_data: Option<&PortData>,
    content_block: Block,
) {
    let mut info_lines: Vec<Line> = Vec::new();
    
    if let Some(p) = port_data {
        // Port status and basic info
        let status_style = match p.state {
            PortState::Free => Style::default(),
            PortState::OccupiedByThis => Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            PortState::OccupiedByOther => Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        };

        let status_text = match p.state {
            PortState::Free => lang().index.port_state_free.clone(),
            PortState::OccupiedByThis => lang().index.port_state_owned.clone(),
            PortState::OccupiedByOther => lang().index.port_state_other.clone(),
        };

        // Basic port information
        info_lines.push(Line::from(vec![
            Span::styled("Port: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(p.port_name.clone())
        ]));
        
        if !p.port_type.is_empty() {
            info_lines.push(Line::from(vec![
                Span::styled("Type: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(p.port_type.clone())
            ]));
        }

        info_lines.push(Line::from(vec![
            Span::styled("Status: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(status_text, status_style)
        ]));

        // Show runtime configuration if port is active
        if let Some(runtime) = &p.runtime {
            info_lines.push(Line::from(""));
            info_lines.push(Line::from(Span::styled("Configuration:", Style::default().add_modifier(Modifier::BOLD))));
            
            let cfg = &runtime.current_cfg;
            info_lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Baud Rate: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(cfg.baud.to_string())
            ]));
            
            info_lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Data Bits: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(cfg.data_bits.to_string())
            ]));
            
            let parity_str = match cfg.parity {
                serialport::Parity::None => "None",
                serialport::Parity::Even => "Even", 
                serialport::Parity::Odd => "Odd",
            };
            info_lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Parity: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(parity_str)
            ]));
            
            info_lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Stop Bits: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(cfg.stop_bits.to_string())
            ]));
        }

        // USB/Hardware information if available
        if p.extra.vid.is_some() || p.extra.pid.is_some() {
            info_lines.push(Line::from(""));
            info_lines.push(Line::from(Span::styled("Hardware:", Style::default().add_modifier(Modifier::BOLD))));
            
            if let (Some(vid), Some(pid)) = (p.extra.vid, p.extra.pid) {
                info_lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("VID:PID: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(format!("{:04x}:{:04x}", vid, pid))
                ]));
            }
            
            if let Some(serial) = &p.extra.serial {
                info_lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("Serial: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(serial.clone())
                ]));
            }
            
            if let Some(manufacturer) = &p.extra.manufacturer {
                info_lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("Manufacturer: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(manufacturer.clone())
                ]));
            }
            
            if let Some(product) = &p.extra.product {
                info_lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("Product: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(product.clone())
                ]));
            }
        }

        // Log statistics
        if !p.logs.is_empty() {
            info_lines.push(Line::from(""));
            info_lines.push(Line::from(Span::styled("Logging:", Style::default().add_modifier(Modifier::BOLD))));
            
            let total_logs = p.logs.len();
            let send_count = p.logs.iter().filter(|log| log.raw.contains("Send") || log.raw.contains("TX")).count();
            let recv_count = p.logs.iter().filter(|log| log.raw.contains("Recv") || log.raw.contains("RX")).count();
            
            info_lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Total Entries: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(total_logs.to_string())
            ]));
            
            if send_count > 0 {
                info_lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("Sent: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(send_count.to_string(), Style::default().fg(Color::Green))
                ]));
            }
            
            if recv_count > 0 {
                info_lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("Received: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(recv_count.to_string(), Style::default().fg(Color::Yellow))
                ]));
            }
        }
    } else {
        info_lines.push(Line::from("Port data not available"));
    }

    render_boxed_paragraph(f, area, info_lines, 0, Some(content_block), true);
}
