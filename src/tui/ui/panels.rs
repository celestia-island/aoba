use ratatui::{
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Tabs},
    widgets::{BorderType, Borders},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    i18n::lang,
    protocol::status::{Focus, Status},
};

pub fn render_panels(f: &mut Frame, area: Rect, app: &Status) {
    // main area split horizontally into left/right panels (bottom bar handled by bottom::render_bottom)
    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .margin(0)
        .constraints([
            ratatui::layout::Constraint::Percentage(40),
            ratatui::layout::Constraint::Percentage(60),
        ])
        .split(area);

    // left: port table with name left-aligned and state right-aligned
    // Build lines manually so we can right-align the state column.
    let left_rect = chunks[0];
    let width = left_rect.width as usize;
    let mut lines: Vec<Line> = Vec::new();
    for (i, p) in app.ports.iter().enumerate() {
        let name = p.port_name.clone();
        let state = app
            .port_states
            .get(i)
            .cloned()
            .unwrap_or(crate::protocol::status::PortState::Free);
        // base style for state text
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

        // compute padding to right-align state using display width (handles CJK)
        // account for Block borders: the content area is two chars narrower
        let inner_width = width.saturating_sub(2);
        let name_w = UnicodeWidthStr::width(name.as_str());
        let state_w = UnicodeWidthStr::width(state_text.as_str());
        // leave at least one space between name and state
        let pad = if inner_width > name_w + state_w {
            inner_width - name_w - state_w
        } else {
            1
        };
        let spacer = " ".repeat(pad);

        // style port name based on occupancy
        let name_span = match state {
            crate::protocol::status::PortState::OccupiedByThis => Span::styled(
                name,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            crate::protocol::status::PortState::OccupiedByOther => Span::styled(
                name,
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ),
            _ => Span::raw(name),
        };
        let mut spans = vec![name_span, Span::raw(spacer)];
        spans.push(Span::styled(state_text, state_style));

        // if selected, apply background highlight across the whole line by wrapping styled spans
        if i == app.selected {
            // apply a background style to each span by re-styling
            let mut styled_spans: Vec<Span> = Vec::new();
            for sp in spans.into_iter() {
                let s = sp.style;
                let combined = s.bg(Color::LightGreen).add_modifier(Modifier::BOLD);
                styled_spans.push(Span::styled(sp.content, combined));
            }
            lines.push(Line::from(styled_spans));
        } else {
            lines.push(Line::from(spans));
        }
    }

    // If no ports, show a placeholder
    let left_content = if lines.is_empty() {
        vec![Line::from(Span::raw(lang().no_com_ports.clone()))]
    } else {
        lines
    };

    let mut left_block = Block::default().borders(Borders::ALL);
    // only style the title text (not the leading space) when left panel is focused
    let title_text = lang().com_ports.as_str();
    let left_title = if matches!(app.focus, Focus::Left) {
        Line::from(vec![
            Span::raw(" "),
            Span::styled(
                title_text,
                Style::default().bg(Color::Gray).fg(Color::White),
            ),
        ])
    } else {
        Line::from(vec![Span::raw(" "), Span::raw(title_text)])
    };
    left_block = left_block.title(left_title);

    let mut paragraph = Paragraph::new(left_content).block(left_block);
    // color convention: SELECTED panel should be darker; UNSELECTED lighter/white
    if matches!(app.focus, Focus::Right) {
        // left is NOT selected
        paragraph = paragraph.style(Style::default().fg(Color::White));
    } else {
        // left is selected
        paragraph = paragraph.style(Style::default().fg(Color::DarkGray));
    }
    f.render_widget(paragraph, chunks[0]);
    // compute whether selected port is occupied by this app early so we can adjust layout
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
        .split(chunks[1]);

    // Tabs header will be rendered as three blocks (b0/b1/b2) below

    // Use native Tabs widget from ratatui
    // build titles as lines (Vec<Span>) which Tabs accepts
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

    // Only show tabs when the selected port is occupied by THIS application.
    if selected_state == crate::protocol::status::PortState::OccupiedByThis {
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
    let right_title = if matches!(app.focus, Focus::Right) {
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
    let cb_style = if matches!(app.focus, Focus::Right) {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };
    content_block = content_block.style(cb_style);

    // main content (middle chunk)
    // compute optional serial parameters to show when we hold the port
    let mut serial_params: String = String::new();
    if selected_state == crate::protocol::status::PortState::OccupiedByThis {
        if let Some(slot) = app.port_handles.get(app.selected) {
            if let Some(handle) = slot.as_ref() {
                // attempt to read common settings; fall back to '未知' on error
                let baud = handle
                    .baud_rate()
                    .map(|b| b.to_string())
                    .unwrap_or("未知".to_string());
                let stop = handle
                    .stop_bits()
                    .map(|s| format!("{:?}", s))
                    .unwrap_or("未知".to_string());
                serial_params = format!("\n串口参数:\n- 波特率: {}\n- 停止位: {}", baud, stop);
            } else {
                serial_params = "\n串口参数: 未知".to_string();
            }
        } else {
            serial_params = "\n串口参数: 未知".to_string();
        }
    }

    let main_content = if app.ports.is_empty() {
        Paragraph::new(lang().no_com_ports.as_str()).block(content_block)
    } else {
        let p = &app.ports[app.selected];
        // richer placeholder per mode using i18n mode titles
        let text = match app.right_mode {
            crate::protocol::status::RightMode::Master => format!(
                "{}\n--------------------\n{} {}\n{} {:?}\n\n{}:\n- {}\n- {}\n\n{}: {}",
                lang().master_mode,
                lang().name_label,
                p.port_name,
                lang().type_label,
                p.port_type,
                lang().details_placeholder, // reuse placeholder label for section
                "发送读写请求",
                "查看响应/日志",
                "说明",
                "该模式用于以主站角色主动轮询或下发指令。",
            ),
            crate::protocol::status::RightMode::SlaveStack => format!(
                "{}\n--------------------\n{} {}\n{} {:?}\n\n{}:\n- {}\n- {}\n\n{}: {}",
                lang().slave_mode,
                lang().name_label,
                p.port_name,
                lang().type_label,
                p.port_type,
                lang().details_placeholder,
                "配置从站寄存器映射",
                "模拟从站响应",
                "说明",
                "该模式用于模拟多个从站堆栈，用于被其他主站轮询。",
            ),
            crate::protocol::status::RightMode::Listen => format!(
                "{}\n--------------------\n{} {}\n{} {:?}\n\n{}:\n- {}\n- {}\n\n{}: {}",
                lang().listen_mode,
                lang().name_label,
                p.port_name,
                lang().type_label,
                p.port_type,
                lang().details_placeholder,
                "监听总线数据",
                "实时显示原始帧/日志",
                "说明",
                "该模式用于被动监听，不主动发送请求。",
            ),
        };
        // append serial params when applicable
        let merged = format!("{}{}", text, serial_params);
        Paragraph::new(merged).block(content_block)
    };

    f.render_widget(main_content, right_chunks[1]);

    // bottom status handled by bottom::render_bottom from parent
}
