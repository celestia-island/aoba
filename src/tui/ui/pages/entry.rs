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

use crate::{
    tui::{
        input::Action,
        ui::pages::{pull, slave},
    },
    {i18n::lang, protocol::status::Status},
};

/// Provide bottom bar hints for the entry view (when used as full-area or main view).
pub fn page_bottom_hints(app: &Status) -> Vec<String> {
    let mut hints: Vec<String> = Vec::new();
    // first hint: switching COM ports with Up/Down or k/j
    hints.push(lang().hint_move_vertical.as_str().to_string());
    // second hint: press 'l' to enter subpage
    hints.push(lang().hint_enter_subpage.as_str().to_string());

    // if selected port is occupied by this app and no subpage overlay is active,
    // add mode menu hint
    let state = app
        .port_states
        .get(app.selected)
        .cloned()
        .unwrap_or(crate::protocol::status::PortState::Free);
    if state == crate::protocol::status::PortState::OccupiedByThis && app.active_subpage.is_none() {
        hints.push(lang().hint_mode_menu.as_str().to_string());
    }

    // Append quit hint only when allowed (mirror global rule)
    let in_subpage_editing = app
        .subpage_form
        .as_ref()
        .map(|f| f.editing)
        .unwrap_or(false);
    let can_quit = app.active_subpage.is_none() && !app.mode_selector_active && !in_subpage_editing;
    if can_quit {
        hints.push(lang().press_q_quit.as_str().to_string());
    }
    hints
}

/// Page-level key mapping for entry. Return Some(Action) if page wants to map the key.
pub fn map_key(_key: KeyEvent, _app: &Status) -> Option<Action> {
    // entry does not add extra mappings; let global mapping handle it
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
    for (i, p) in app.ports.iter().enumerate() {
        let name = p.port_name.clone();
        let state = app
            .port_states
            .get(i)
            .cloned()
            .unwrap_or(crate::protocol::status::PortState::Free);
        let (state_text, state_style) = match state {
            crate::protocol::status::PortState::Free => {
                (lang().port_state_free.clone(), Style::default())
            }
            crate::protocol::status::PortState::OccupiedByThis => (
                lang().port_state_owned.clone(),
                Style::default().fg(Color::Green),
            ),
            crate::protocol::status::PortState::OccupiedByOther => (
                lang().port_state_other.clone(),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ),
        };

        // Prefix: two chars to avoid shifting when navigating. Selected shows '> ', else two spaces.
        let prefix = if i == app.selected { "> " } else { "  " };
        let inner = width.saturating_sub(2);
        // account for prefix width in name column
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
        if i == app.selected {
            // highlight entire row including prefix
            let styled = spans
                .into_iter()
                .map(|sp| Span::styled(sp.content, Style::default().bg(Color::LightGreen)))
                .collect::<Vec<_>>();
            lines.push(Line::from(styled));
        } else {
            lines.push(Line::from(spans));
        }
    }

    // extra labels (anchor to bottom of left panel)
    let extras = vec![
        lang().refresh_label.clone(),
        lang().manual_specify_label.clone(),
    ];
    // compute inner vertical space (accounting for borders)
    let inner_h = left.height.saturating_sub(2) as usize;
    // how many lines will be occupied by ports currently
    let used = lines.len();
    let extras_len = extras.len();
    // number of blank lines to insert so that extras appear at the bottom
    let pad_lines = if inner_h > used + extras_len {
        inner_h - used - extras_len
    } else {
        0
    };
    for _ in 0..pad_lines {
        lines.push(Line::from(Span::raw("")));
    }

    for (j, lbl) in extras.into_iter().enumerate() {
        let idx = app.ports.len() + j;
        let prefix = if idx == app.selected { "> " } else { "  " };
        let spans = vec![Span::raw(prefix), Span::raw(lbl)];
        if idx == app.selected {
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
        .title(Span::raw(format!(" {}", lang().com_ports.as_str())));
    let left_para = Paragraph::new(lines).block(left_block);
    f.render_widget(left_para, left);

    // RIGHT: content (no tabs). When selected port is OccupiedByThis and not in a subpage,
    // details should occupy the full right area and include serial parameters.
    let selected_state = app
        .port_states
        .get(app.selected)
        .cloned()
        .unwrap_or(crate::protocol::status::PortState::Free);

    // If a subpage is active, delegate the entire right area to it.
    if let Some(sub) = app.active_subpage {
        match sub {
            crate::protocol::status::PortMode::Master => slave::render_slave(f, right, app),
            crate::protocol::status::PortMode::SlaveStack => pull::render_pull(f, right, app),
        }
        return;
    }

    let content_block = Block::default()
        .borders(Borders::ALL)
        .title(Span::raw(format!(" {}", lang().details.as_str())));

    // Build the content taking full height of the right area.
    let content = if app.ports.is_empty() {
        Paragraph::new(lang().no_com_ports.as_str()).block(content_block)
    } else {
        let special_base = app.ports.len();
        if app.selected >= special_base {
            let rel = app.selected - special_base;
            let txt = if rel == 0 {
                lang().refresh_label.as_str().to_string()
            } else {
                lang().manual_specify_label.as_str().to_string()
            };
            Paragraph::new(txt).block(content_block)
        } else {
            let p = &app.ports[app.selected];

            // Gather serial parameters if handle exists
            let mut baud = lang().serial_unknown.clone();
            let mut stop = lang().serial_unknown.clone();
            let mut data_bits = lang().serial_unknown.clone();
            let mut parity = lang().serial_unknown.clone();
            if let Some(slot) = app.port_handles.get(app.selected) {
                if let Some(handle) = slot.as_ref() {
                    baud = handle.baud_rate().map(|b| b.to_string()).unwrap_or(baud);
                    stop = handle
                        .stop_bits()
                        .map(|s| match s {
                            serialport::StopBits::One => "1".to_string(),
                            serialport::StopBits::Two => "2".to_string(),
                        })
                        .unwrap_or(stop);
                    data_bits = handle
                        .data_bits()
                        .map(|d| match d {
                            serialport::DataBits::Five => "5".to_string(),
                            serialport::DataBits::Six => "6".to_string(),
                            serialport::DataBits::Seven => "7".to_string(),
                            serialport::DataBits::Eight => "8".to_string(),
                        })
                        .unwrap_or(data_bits);
                    parity = handle
                        .parity()
                        .map(|p| match p {
                            serialport::Parity::None => lang().parity_none.clone(),
                            serialport::Parity::Even => lang().parity_even.clone(),
                            serialport::Parity::Odd => lang().parity_odd.clone(),
                        })
                        .unwrap_or(parity);
                }
            }

            // Localized status text and style
            let status_text = match selected_state {
                crate::protocol::status::PortState::Free => lang().port_state_free.clone(),
                crate::protocol::status::PortState::OccupiedByThis => {
                    lang().port_state_owned.clone()
                }
                crate::protocol::status::PortState::OccupiedByOther => {
                    lang().port_state_other.clone()
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

            // Prepare label/value pairs (value strings already include leading space where needed)
            let mut pairs: Vec<(String, String, Option<Style>)> = Vec::new();
            pairs.push((
                lang().label_port.as_str().to_string(),
                format!("{}", p.port_name),
                None,
            ));
            pairs.push((
                lang().label_type.as_str().to_string(),
                format!("{:?}", p.port_type),
                None,
            ));
            pairs.push((
                lang().label_status.as_str().to_string(),
                format!("{}", status_text),
                Some(status_style),
            ));

            // If occupied by this app, show current right mode (localized)
            if selected_state == crate::protocol::status::PortState::OccupiedByThis {
                let mode_text = match app.port_mode {
                    crate::protocol::status::PortMode::Master => lang().master_mode.clone(),
                    crate::protocol::status::PortMode::SlaveStack => lang().slave_mode.clone(),
                };
                pairs.push((lang().label_mode.as_str().to_string(), mode_text, None));
            }

            // Serial parameter pairs
            pairs.push((lang().label_baud.as_str().to_string(), baud, None));
            pairs.push((lang().label_data_bits.as_str().to_string(), data_bits, None));
            pairs.push((lang().label_parity.as_str().to_string(), parity, None));
            pairs.push((lang().label_stop_bits.as_str().to_string(), stop, None));

            // Compute max label width (without indent)
            let indent = "  ";
            let max_label_w = pairs
                .iter()
                .map(|(lbl, _, _)| UnicodeWidthStr::width(lbl.as_str()))
                .max()
                .unwrap_or(0usize);

            for (lbl, val, maybe_style) in pairs.into_iter() {
                let lbl_w = UnicodeWidthStr::width(lbl.as_str());
                // add 4 extra spaces to increase distance between label and value
                let pad = if max_label_w >= lbl_w {
                    max_label_w - lbl_w + 5
                } else {
                    5
                };
                let spacer = " ".repeat(pad);
                let label_span = Span::styled(
                    format!("{}{}", indent, lbl),
                    Style::default().add_modifier(Modifier::BOLD),
                );
                match maybe_style {
                    Some(s) => info_lines.push(Line::from(vec![
                        label_span,
                        Span::raw(spacer),
                        Span::styled(format!("{}", val), s),
                    ])),
                    None => info_lines.push(Line::from(vec![
                        label_span,
                        Span::raw(spacer),
                        Span::raw(format!("{}", val)),
                    ])),
                }
            }

            Paragraph::new(info_lines)
                .block(content_block)
                .wrap(ratatui::widgets::Wrap { trim: true })
        }
    };
    f.render_widget(content, right);

    // If mode selector overlay is active, render it via reusable component
    if app.mode_selector_active {
        crate::tui::ui::components::mode_selector::render_mode_selector(f, app.mode_selector_index);
    }
}

/// Handle key events when entry is used as a full-area subpage (listen). Return true if consumed.
pub fn handle_subpage_key(
    _key: crossterm::event::KeyEvent,
    _app: &mut crate::protocol::status::Status,
) -> bool {
    use crossterm::event::KeyCode as KC;
    // Provide simple handling for listen mode: consume Up/Down/Enter to avoid bubbling
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
