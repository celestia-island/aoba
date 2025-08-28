use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Tabs},
    widgets::{BorderType, Borders},
};
use unicode_width::UnicodeWidthStr;

use crate::{i18n::lang, protocol::status::Status};

fn describe_port(p: &serialport::SerialPortInfo) -> String {
    // Avoid platform-dependent destructuring of SerialPortType. Use Debug representation
    // which is stable across platforms.
    format!("端口: {}\n类型: {:?}\n", p.port_name, p.port_type)
}

pub fn render_details(f: &mut Frame, area: Rect, app: &Status) {
    // Right: show details for selected port or for virtual entries
    // If a virtual item is selected, render special info
    let selected_state = app
        .port_states
        .get(app.selected)
        .cloned()
        .unwrap_or(crate::protocol::status::PortState::Free);

    // Right side: split into top (tabs) and bottom (content)
    let top_len = if selected_state == crate::protocol::status::PortState::OccupiedByThis {
        ratatui::layout::Constraint::Length(3)
    } else {
        // hide the top area entirely when not occupied
        ratatui::layout::Constraint::Length(0)
    };

    let right_chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .margin(0)
        .constraints([
            top_len,                             // top: tabs (dynamic)
            ratatui::layout::Constraint::Min(0), // middle: main content
        ])
        .split(area);

    // Only show tabs when the selected port is occupied by THIS application.
    if selected_state == crate::protocol::status::PortState::OccupiedByThis {
        // Determine selected tab index from current mode
        let tab_index = match app.right_mode {
            crate::protocol::status::RightMode::Master => 0,
            crate::protocol::status::RightMode::SlaveStack => 1,
            crate::protocol::status::RightMode::Listen => 2,
        };

        // Build titles as Lines with padding; unselected have gray background, selected will use highlight_style
        let titles = [
            lang().tab_master.as_str(),
            lang().tab_slave.as_str(),
            lang().tab_listen.as_str(),
        ]
        .iter()
        .map(|t| {
            let label = format!("  {}  ", t);
            Line::from(Span::styled(
                label,
                Style::default().bg(Color::DarkGray).fg(Color::White),
            ))
        })
        .collect::<Vec<Line>>();

        let tabs = Tabs::new(titles)
            .select(tab_index)
            .padding("", "")
            .divider(" ")
            // normal (unselected) style already painted per-title; also ensure overall style doesn't override
            .style(Style::default())
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(0, 128, 0))
                    .add_modifier(Modifier::BOLD),
            );

        // Center tabs vertically inside the top 3-line area: leave one line above and below
        let tabs_inner = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .margin(0)
            .constraints([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Length(1),
            ])
            .split(right_chunks[0]);

        // Compute total width of labels (include per-label padding and divider) for horizontal centering
        let label_texts = [
            lang().tab_master.as_str(),
            lang().tab_slave.as_str(),
            lang().tab_listen.as_str(),
        ];
        let mut total_w: usize = 0;
        for (i, t) in label_texts.iter().enumerate() {
            let label = format!("  {}  ", t);
            total_w += UnicodeWidthStr::width(label.as_str());
            if i > 0 {
                // divider width (single space)
                total_w += 1;
            }
        }

        let area = tabs_inner[1];
        let area_w = area.width as usize;
        let left_pad = if area_w > total_w {
            (area_w - total_w) / 2
        } else {
            0
        };
        let render_w = std::cmp::min(total_w, area_w) as u16;
        let centered = Rect::new(area.x + left_pad as u16, area.y, render_w, area.height);
        f.render_widget(tabs, centered);
    }

    // Bottom: content area
    let mut content_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain);
    // only style the title with gray background when right panel is focused
    let right_text = lang().details.as_str();
    let right_title = if matches!(app.focus, crate::protocol::status::Focus::Right) {
        Line::from(vec![
            Span::raw(" "),
            Span::styled(
                right_text,
                Style::default().bg(Color::Gray).fg(Color::White),
            ),
        ])
    } else {
        Line::from(vec![Span::raw(" "), Span::raw(right_text)])
    };
    content_block = content_block.title(right_title);
    // set only foreground color for content (not background)
    let cb_style = if matches!(app.focus, crate::protocol::status::Focus::Right) {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };
    content_block = content_block.style(cb_style);

    // main content (middle chunk)
    let main_content = if app.ports.is_empty() {
        Paragraph::new(lang().no_com_ports.as_str()).block(content_block)
    } else {
        // if selected index is beyond ports, treat as special virtual items
        let special_base = app.ports.len();
        if app.selected >= special_base {
            // index mapping: base -> Refresh, base+1 -> Manual specify
            let rel = app.selected - special_base;
            let special_text = if rel == 0 {
                // Refresh description: minimal label
                lang().refresh_label.as_str().to_string()
            } else {
                // Manual specify
                #[cfg(target_os = "linux")]
                let manual_note = lang().manual_specify_linux_note.as_str();
                #[cfg(not(target_os = "linux"))]
                let manual_note = lang().manual_specify_unsupported.as_str();
                format!(
                    "{}\n\n{}",
                    lang().manual_specify_label.as_str(),
                    manual_note
                )
            };

            Paragraph::new(special_text).block(content_block)
        } else {
            let p = &app.ports[app.selected];

            // device summary common to read-only cases
            let device_info = describe_port(p);

            let text = match selected_state {
                crate::protocol::status::PortState::Free => {
                    // not opened by anyone
                    format!(
                        "{}\n状态: 未连接\n\n提示: 选中此端口并打开以进入配置/操作界面。",
                        device_info
                    )
                }
                crate::protocol::status::PortState::OccupiedByOther => {
                    // occupied by another program: show only device/port info
                    format!(
                    "{}\n状态: 已被其他程序占用\n\n说明: 该端口当前被其他程序占用，无法选择运行模式。",
                    device_info
                )
                }
                crate::protocol::status::PortState::OccupiedByThis => {
                    // connected by this app: show mode info + serial params (editable via controls)
                    // fetch serial params
                    let mut baud = "未知".to_string();
                    let mut stop = "未知".to_string();
                    if let Some(slot) = app.port_handles.get(app.selected) {
                        if let Some(handle) = slot.as_ref() {
                            baud = handle.baud_rate().map(|b| b.to_string()).unwrap_or(baud);
                            stop = handle
                                .stop_bits()
                                .map(|s| format!("{:?}", s))
                                .unwrap_or(stop);
                        }
                    }

                    let mode_name = match app.right_mode {
                        crate::protocol::status::RightMode::Master => lang().master_mode.as_str(),
                        crate::protocol::status::RightMode::SlaveStack => {
                            lang().slave_mode.as_str()
                        }
                        crate::protocol::status::RightMode::Listen => lang().listen_mode.as_str(),
                    };

                    format!(
                    "{}\n状态: 已被本程序占用\n\n当前模式: {}\n\n串口参数:\n- 波特率: {}\n- 停止位: {}\n\n说明: 可临时在顶部选择运行模式；使用快捷键调整波特率/停止位（未实现变化处理时，请通过菜单/快捷键修改）。",
                    device_info, mode_name, baud, stop
                )
                }
            };
            Paragraph::new(text).block(content_block)
        }
    };

    f.render_widget(main_content, right_chunks[1]);
}
