// Clean single implementation of the entry page (ports list + right details / subpage delegate)
use crossterm::event::KeyEvent;
use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Borders,
    widgets::{Block, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::{i18n::lang, protocol::status::Status, tui::input::Action};

/// Provide bottom bar hints for the entry view (when used as full-area or main view).
pub fn page_bottom_hints(app: &Status) -> Vec<String> {
    let mut hints: Vec<String> = Vec::new();
    // First hint: switching COM ports with Up / Down or k / j
    hints.push(lang().hotkeys.hint_move_vertical.as_str().to_string());
    // Second hint: press 'l' to enter subpage
    hints.push(lang().hotkeys.hint_enter_subpage.as_str().to_string());

    // Append quit hint only when allowed (mirror global rule)
    let in_subpage_editing = app
        .page
        .subpage_form
        .as_ref()
        .map(|f| f.editing)
        .unwrap_or(false);
    let can_quit = !app.page.subpage_active && !in_subpage_editing;
    if can_quit {
        hints.push(lang().hotkeys.press_q_quit.as_str().to_string());
    }
    hints
}

/// Page-level key mapping for entry. Return Some(Action) if page wants to map the key.
pub fn map_key(_key: KeyEvent, _app: &Status) -> Option<Action> {
    // Entry does not add extra mappings; let global mapping handle it
    None
}

pub fn render_entry(f: &mut Frame, area: Rect, app: &mut Status) {
    // Horizontal split: left ports | right details
    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .margin(0)
        .constraints([
            ratatui::layout::Constraint::Percentage(40),
            ratatui::layout::Constraint::Percentage(60),
        ])
        .split(area);

    let left = chunks[0];
    let right = chunks[1];

    // LEFT: ports list
    let width = left.width as usize;
    let mut lines: Vec<Line> = Vec::new();
    for (i, p) in app.ports.list.iter().enumerate() {
        let name = p.port_name.clone();
        let state = app
            .ports
            .states
            .get(i)
            .map(|s| s.port_state.clone())
            .unwrap_or(crate::protocol::status::PortState::Free);
        let (state_text, state_style) = match state {
            crate::protocol::status::PortState::Free => {
                (lang().index.port_state_free.clone(), Style::default())
            }
            crate::protocol::status::PortState::OccupiedByThis => (
                lang().index.port_state_owned.clone(),
                Style::default().fg(Color::Green),
            ),
            crate::protocol::status::PortState::OccupiedByOther => (
                lang().index.port_state_other.clone(),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ),
        };

        // Prefix: two chars to avoid shifting when navigating. Selected shows '> ', else two spaces.
        let prefix = if i == app.page.selected { "> " } else { "  " };
        let inner = width.saturating_sub(2);
        // Account for prefix width in name column
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
        if i == app.page.selected {
            // Highlight entire row including prefix
            let styled = spans
                .into_iter()
                .map(|sp| Span::styled(sp.content, Style::default().bg(Color::LightGreen)))
                .collect::<Vec<_>>();
            lines.push(Line::from(styled));
        } else {
            lines.push(Line::from(spans));
        }
    }

    // Extra labels (anchor to bottom of left panel)
    let extras = vec![
        lang().index.refresh_action.clone(),
        lang().index.manual_specify_label.clone(),
        lang().index.about_label.clone(),
    ];
    // Compute inner vertical space (accounting for borders)
    let inner_h = left.height.saturating_sub(2) as usize;
    // How many lines will be occupied by ports currently
    let used = lines.len();
    let extras_len = extras.len();
    // Number of blank lines to insert so that extras appear at the bottom
    let pad_lines = if inner_h > used + extras_len {
        inner_h - used - extras_len
    } else {
        0
    };
    for _ in 0..pad_lines {
        lines.push(Line::from(Span::raw("")));
    }

    for (j, lbl) in extras.into_iter().enumerate() {
        let idx = app.ports.list.len() + j;
        let prefix = if idx == app.page.selected { "> " } else { "  " };
        let spans = vec![Span::raw(prefix), Span::raw(lbl)];
        if idx == app.page.selected {
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
    f.render_widget(left_para, left);

    // RIGHT: content (no tabs). When selected port is OccupiedByThis and not in a subpage,
    // Details should occupy the full right area and include serial parameters.
    let selected_state = app
        .ports
        .states
        .get(app.page.selected)
        .map(|s| s.port_state.clone())
        .unwrap_or(crate::protocol::status::PortState::Free);

    // If a subpage is active, delegate the entire right area to it.
    if app.page.subpage_active {
        // Unified ModBus page now handled elsewhere; entry no longer renders legacy subpages.
        return;
    }

    let content_block = Block::default()
        .borders(Borders::ALL)
        .title(Span::raw(format!(" {}", lang().index.details.as_str())));

    // Build the content taking full height of the right area.
    let content = if app.ports.list.is_empty() {
        Paragraph::new(lang().index.no_com_ports.as_str()).block(content_block)
    } else {
        let special_base = app.ports.list.len();
        if app.page.selected >= special_base {
            let rel = app.page.selected - special_base;
            if rel == 0 {
                // Refresh action item: show last scan summary
                let mut lines: Vec<Line> = Vec::new();
                lines.push(Line::from(lang().index.refresh_action.as_str()));
                // Quick scan hint
                lines.push(Line::from(Span::styled(
                    lang().index.scan_quick_hint.as_str(),
                    Style::default().fg(Color::LightBlue),
                )));
                if let Some(ts) = app.scan.last_scan_time {
                    lines.push(Line::from(format!(
                        "{} {}",
                        lang().index.scan_last_header.as_str(),
                        ts.format("%Y-%m-%d %H:%M:%S")
                    )));
                } else {
                    lines.push(Line::from(lang().index.scan_none.as_str()));
                }
                if app.scan.last_scan_info.is_empty() {
                    lines.push(Line::from(format!("({})", lang().index.scan_none.as_str())));
                } else {
                    lines.push(Line::from(lang().index.scan_raw_header.as_str()));
                    for l in app.scan.last_scan_info.lines().take(100) {
                        // Cap lines to avoid overflow
                        if l.starts_with("ERROR:") {
                            lines
                                .push(Line::from(Span::styled(l, Style::default().fg(Color::Red))));
                        } else {
                            lines.push(Line::from(l));
                        }
                    }
                    if app.scan.last_scan_info.len() > 100 {
                        lines.push(Line::from(format!(
                            "... ({} {})",
                            app.scan.last_scan_info.len() - 100,
                            lang().index.scan_truncated_suffix.as_str()
                        )));
                    }
                }
                Paragraph::new(lines).block(content_block)
            } else if rel == 1 {
                Paragraph::new(lang().index.manual_specify_label.as_str()).block(content_block)
            } else if rel == 2 {
                // About page selected; show a compact preview that reuses about rendering logic
                // Build an AboutCache snapshot if available by peeking the internal cache (best-effort)
                crate::tui::ui::pages::about::render_about(f, right, app);
                return;
            } else {
                Paragraph::new(lang().index.manual_specify_label.as_str()).block(content_block)
            }
        } else {
            let p = &app.ports.list[app.page.selected];
            let extra = app
                .ports
                .extras
                .get(app.page.selected)
                .map(|e| e.port_extra.clone())
                .unwrap_or_default();

            // Prefer runtime's current_cfg (live synchronized config). If not occupied we hide these fields.
            let runtime_cfg = if let Some(r) = app.ports.runtimes.get(app.page.selected) {
                r.runtime.as_ref().map(|rt| rt.current_cfg.clone())
            } else {
                None
            };

            // Localized status text and style
            let status_text = match selected_state {
                crate::protocol::status::PortState::Free => lang().index.port_state_free.clone(),
                crate::protocol::status::PortState::OccupiedByThis => {
                    lang().index.port_state_owned.clone()
                }
                crate::protocol::status::PortState::OccupiedByOther => {
                    lang().index.port_state_other.clone()
                }
            };

            let status_style = match selected_state {
                crate::protocol::status::PortState::Free => Style::default(),
                crate::protocol::status::PortState::OccupiedByThis => Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
                crate::protocol::status::PortState::OccupiedByOther => Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            };

            // Build styled lines and align values into a right-hand column.
            let mut info_lines: Vec<Line> = Vec::new();

            // Prepare label / value pairs (value strings already include leading space where needed)
            let mut pairs: Vec<(String, String, Option<Style>)> = Vec::new();
            pairs.push((
                lang().protocol.common.label_port.as_str().to_string(),
                p.port_name.to_string(),
                None,
            ));
            // Render type on its own line
            let type_val = format!("{:?}", p.port_type);
            pairs.push((
                lang().protocol.common.label_type.as_str().to_string(),
                type_val,
                None,
            ));
            // Mapping code: platform-neutral label. On Windows render GUID; on Unix-like render vid/pid.
            // Use localized label `label_mapping_code` and localized placeholder `mapping_none`.
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
            // Avoid adding a separate USB row when mapping already displays vid/pid on non-Windows.
            let mapping_consumes_usb =
                !cfg!(windows) && (extra.vid.is_some() || extra.pid.is_some());
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
            // Current per-port application mode (ModBus / MQTT)
            if matches!(
                selected_state,
                crate::protocol::status::PortState::OccupiedByThis
            ) {
                let mode_label = match app.page.app_mode {
                    crate::protocol::status::AppMode::Modbus => {
                        lang().protocol.common.mode_modbus.as_str()
                    }
                    crate::protocol::status::AppMode::Mqtt => {
                        lang().protocol.common.mode_mqtt.as_str()
                    }
                };
                pairs.push((
                    lang().protocol.common.label_mode.as_str().to_string(),
                    mode_label.to_string(),
                    None,
                ));
            }

            // Mode always unified; hide previous master / slave mode line.

            if matches!(
                selected_state,
                crate::protocol::status::PortState::OccupiedByThis
            ) {
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

            // Compute max label width (without indent)
            let indent = "  ";
            let max_label_w = pairs
                .iter()
                .map(|(lbl, _, _)| UnicodeWidthStr::width(lbl.as_str()))
                .max()
                .unwrap_or(0usize);

            for (lbl, val, maybe_style) in pairs.into_iter() {
                let lbl_w = UnicodeWidthStr::width(lbl.as_str());
                // Pad the label itself to the max label width so the value column always lines up.
                let fill = max_label_w.saturating_sub(lbl_w);
                let padded_label = format!("{indent}{}{}", lbl, " ".repeat(fill));
                // Fixed small gap between label area and value
                let spacer = " ".repeat(5);
                let label_span =
                    Span::styled(padded_label, Style::default().add_modifier(Modifier::BOLD));
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

            Paragraph::new(info_lines)
                .block(content_block)
                .wrap(ratatui::widgets::Wrap { trim: true })
        }
    };
    f.render_widget(content, right);

    // Mode selector removed (unified ModBus RTU) – overlay no longer rendered.
}

/// Handle key events when entry is used as a full-area subpage (listen). Return true if consumed.
pub fn handle_subpage_key(
    _key: crossterm::event::KeyEvent,
    _app: &mut crate::protocol::status::Status,
) -> bool {
    use crossterm::event::KeyCode as KC;
    // Provide simple handling for listen mode: consume Up / Down / Enter to avoid bubbling
    match _key.code {
        KC::Up
        | KC::Down
        | KC::Left
        | KC::Right
        | KC::Char('k')
        | KC::Char('j')
        | KC::Char('h')
        | KC::Char('l') => return true,
        KC::Enter => return true,
        _ => {}
    }
    false
}
