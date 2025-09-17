use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    i18n::lang,
    protocol::status::types::{self, port::PortData, port::PortState, ui::SpecialEntry, Status},
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
pub fn render_ports_list(f: &mut Frame, area: Rect, app: &Status, selection: usize) {
    let width = area.width as usize;
    let mut lines: Vec<Line> = Vec::new();
    let default_pd = PortData::default();
    
    for (i, name) in app.ports.order.iter().enumerate() {
        let p = app.ports.map.get(name).unwrap_or(&default_pd);
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

    let extras = SpecialEntry::all()
        .iter()
        .map(|s| match s {
            SpecialEntry::Refresh => lang().index.refresh_action.as_str().to_string(),
            SpecialEntry::ManualSpecify => lang().index.manual_specify_label.as_str().to_string(),
            SpecialEntry::About => lang().index.about_label.as_str().to_string(),
        })
        .collect::<Vec<_>>();
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
        let idx = app.ports.order.len() + j;
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
    f.render_widget(left_para, area);
}

/// Render the right details panel content
pub fn render_details_panel(f: &mut Frame, area: Rect, app: &Status, selection: usize) {
    let selected_state = if selection < app.ports.order.len() {
        let name = &app.ports.order[selection];
        app.ports
            .map
            .get(name)
            .map(|p| p.state.clone())
            .unwrap_or(PortState::Free)
    } else {
        PortState::Free
    };

    let subpage_active = matches!(
        app.page,
        types::Page::ModbusConfig { .. }
            | types::Page::ModbusDashboard { .. }
            | types::Page::ModbusLog { .. }
            | types::Page::About { .. }
    );
    if subpage_active {
        return;
    }

    let content_block = Block::default()
        .borders(Borders::ALL)
        .title(Span::raw(format!(" {}", lang().index.details.as_str())));

    // FIXME: Not all branches are implemented yet.
    if app.ports.order.is_empty() {
        let content = Paragraph::new(lang().index.no_com_ports.as_str()).block(content_block);
        f.render_widget(content, area);
    } else {
        let special_base = app.ports.order.len();
        if selection >= special_base {
            let rel = selection - special_base;
            render_special_entry_content(f, area, app, rel, content_block);
        } else {
            render_port_details(f, area, app, selection, selected_state, content_block);
        }
    }
}

/// Render content for special entries (refresh, manual specify, about)
fn render_special_entry_content(
    f: &mut Frame,
    area: Rect,
    app: &Status,
    rel: usize,
    content_block: Block,
) {
    if rel == 0 {
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(lang().index.refresh_action.as_str()));
        if let Some(ts) = app.temporarily.scan.last_scan_time {
            lines.push(Line::from(format!(
                "{} {}",
                lang().index.scan_last_header.as_str(),
                ts.format("%Y-%m-%d %H:%M:%S")
            )));
        } else {
            lines.push(Line::from(lang().index.scan_none.as_str()));
        }
        if app.temporarily.scan.last_scan_info.is_empty() {
            lines.push(Line::from(format!("({})", lang().index.scan_none.as_str())));
        } else {
            lines.push(Line::from(lang().index.scan_raw_header.as_str()));
            for l in app.temporarily.scan.last_scan_info.lines().take(100) {
                if l.starts_with("ERROR:") {
                    lines
                        .push(Line::from(Span::styled(l, Style::default().fg(Color::Red))));
                } else {
                    lines.push(Line::from(l));
                }
            }
            if app.temporarily.scan.last_scan_info.len() > 100 {
                lines.push(Line::from(format!(
                    "... ({} {})",
                    app.temporarily.scan.last_scan_info.len() - 100,
                    lang().index.scan_truncated_suffix.as_str()
                )));
            }
        }
        let content = Paragraph::new(lines).block(content_block);
        f.render_widget(content, area);
    } else if rel == 1 {
        let content = Paragraph::new(lang().index.manual_specify_label.as_str()).block(content_block);
        f.render_widget(content, area);
    } else if rel == 2 {
        // About page preview - simplified for now
        let content = Paragraph::new("About (TODO: implement preview)").block(content_block);
        f.render_widget(content, area);
    } else {
        let content = Paragraph::new(lang().index.manual_specify_label.as_str()).block(content_block);
        f.render_widget(content, area);
    }
}

/// Render detailed information for a specific port
fn render_port_details(
    f: &mut Frame,
    area: Rect,
    app: &Status,
    selection: usize,
    selected_state: PortState,
    content_block: Block,
) {
    let port_name = app.ports.order.get(selection).cloned().unwrap_or_default();
    let default_pd = PortData::default();
    let p = app.ports.map.get(&port_name).unwrap_or(&default_pd);
    let extra = p.extra.clone();

    let runtime_cfg = p.runtime.as_ref().map(|r| r.current_cfg.clone());

    let status_text = match selected_state {
        PortState::Free => lang().index.port_state_free.clone(),
        PortState::OccupiedByThis => lang().index.port_state_owned.clone(),
        PortState::OccupiedByOther => lang().index.port_state_other.clone(),
    };

    let status_style = match selected_state {
        PortState::Free => Style::default(),
        PortState::OccupiedByThis => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        PortState::OccupiedByOther => Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    };

    let mut info_lines: Vec<Line> = Vec::new();
    let mut pairs: Vec<(String, String, Option<Style>)> = Vec::new();
    pairs.push((
        lang().protocol.common.label_port.as_str().to_string(),
        p.port_name.to_string(),
        None,
    ));
    let type_val = format!("{:?}", p.port_type);
    pairs.push((
        lang().protocol.common.label_type.as_str().to_string(),
        type_val,
        None,
    ));
    let mapping_none = lang().protocol.common.mapping_none.as_str().to_string();
    let mapping_value = if cfg!(windows) {
        extra.guid.clone().unwrap_or_else(|| mapping_none.clone())
    } else if extra.vid.is_some() || extra.pid.is_some() {
        format!(
            "vid:{:04x} pid:{:04x}",
            extra.vid.unwrap_or(0),
            extra.pid.unwrap_or(0)
        )
    } else {
        mapping_none.clone()
    };
    pairs.push((
        lang()
            .protocol
            .common
            .label_mapping_code
            .as_str()
            .to_string(),
        mapping_value,
        None,
    ));
    let mapping_consumes_usb = !cfg!(windows) && (extra.vid.is_some() || extra.pid.is_some());
    if !mapping_consumes_usb && (extra.vid.is_some() || extra.pid.is_some()) {
        let vid_pid = format!(
            "vid:{:04x} pid:{:04x}",
            extra.vid.unwrap_or(0),
            extra.pid.unwrap_or(0)
        );
        pairs.push((
            lang().protocol.common.label_usb.as_str().into(),
            vid_pid,
            None,
        ));
    }
    if let Some(sn) = extra.serial.as_ref() {
        pairs.push((
            lang().protocol.common.label_serial.as_str().into(),
            sn.clone(),
            None,
        ));
    }
    if let Some(m) = extra.manufacturer.as_ref() {
        pairs.push((
            lang().protocol.common.label_manufacturer.as_str().into(),
            m.clone(),
            None,
        ));
    }
    if let Some(prod) = extra.product.as_ref() {
        pairs.push((
            lang().protocol.common.label_product.as_str().into(),
            prod.clone(),
            None,
        ));
    }
    pairs.push((
        lang().protocol.common.label_status.as_str().to_string(),
        status_text.to_string(),
        Some(status_style),
    ));

    if matches!(selected_state, PortState::OccupiedByThis) {
        if let Some(cfg) = runtime_cfg.clone() {
            let baud = cfg.baud.to_string();
            let data_bits = cfg.data_bits.to_string();
            let parity = match cfg.parity {
                serialport::Parity::None => lang().protocol.common.parity_none.clone(),
                serialport::Parity::Even => lang().protocol.common.parity_even.clone(),
                serialport::Parity::Odd => lang().protocol.common.parity_odd.clone(),
            };
            let stop = cfg.stop_bits.to_string();
            pairs.push((
                lang().protocol.common.label_baud.as_str().to_string(),
                baud,
                None,
            ));
            pairs.push((
                lang().protocol.common.label_data_bits.as_str().to_string(),
                data_bits,
                None,
            ));
            pairs.push((
                lang().protocol.common.label_parity.as_str().to_string(),
                parity,
                None,
            ));
            pairs.push((
                lang().protocol.common.label_stop_bits.as_str().to_string(),
                stop,
                None,
            ));
        }
    }

    let indent = "  ";
    let max_label_w = pairs
        .iter()
        .map(|(lbl, _, _)| UnicodeWidthStr::width(lbl.as_str()))
        .max()
        .unwrap_or(0usize);

    for (lbl, val, maybe_style) in pairs.into_iter() {
        let lbl_w = UnicodeWidthStr::width(lbl.as_str());
        let fill = max_label_w.saturating_sub(lbl_w);
        let padded_label = format!("{indent}{}{}", lbl, " ".repeat(fill));
        let spacer = " ".repeat(5);
        let label_span = Span::styled(padded_label, Style::default().add_modifier(Modifier::BOLD));
        match maybe_style {
            Some(s) => info_lines.push(Line::from(vec![
                label_span,
                Span::raw(spacer),
                Span::styled(val.to_string(), s),
            ])),
            None => info_lines.push(Line::from(vec![
                label_span,
                Span::raw(spacer),
                Span::raw(val.to_string()),
            ])),
        }
    }

    let content = Paragraph::new(info_lines)
        .block(content_block)
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(content, area);
}