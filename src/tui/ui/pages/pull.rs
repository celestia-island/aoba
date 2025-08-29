use ratatui::{
    prelude::*,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Tabs},
};

use crate::{i18n::lang, protocol::status::Status};

/// UI for configuring Modbus 从站 (pull) settings for the selected port.
pub fn render_pull(f: &mut Frame, area: Rect, app: &Status) {
    let port_name = if !app.ports.is_empty() && app.selected < app.ports.len() {
        app.ports[app.selected].port_name.clone()
    } else {
        "-".to_string()
    };

    let title_line = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            port_name.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" - {}", lang().tab_slave.as_str())),
    ]);

    let middle_tab = match app.right_mode {
        crate::protocol::status::RightMode::Master => lang().label_master_list.as_str(),
        crate::protocol::status::RightMode::SlaveStack => lang().label_slave_listen.as_str(),
        crate::protocol::status::RightMode::Listen => lang().label_slave_listen.as_str(),
    };
    let tabs = vec!["通信配置", middle_tab, "通信日志"];
    let tab_index = app.subpage_tab_index.min(tabs.len().saturating_sub(1));

    let [header_area, content_area] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(2),
        ratatui::layout::Constraint::Min(0),
    ])
    .areas(area);

    // header: title and tabs
    let [tabs_area, title_area] = ratatui::layout::Layout::horizontal([
        ratatui::layout::Constraint::Min(0),
        ratatui::layout::Constraint::Length(20),
    ])
    .areas(header_area);

    // title
    f.render_widget(Paragraph::new(title_line), title_area);

    // tabs
    let titles = tabs
        .iter()
        .map(|t| Line::from(Span::raw(format!("  {}  ", t))));
    let tabs_widget = Tabs::new(titles).select(tab_index);
    f.render_widget(tabs_widget, tabs_area);

    match tab_index {
        0 => render_pull_config(f, content_area, app),
        1 => match app.right_mode {
            crate::protocol::status::RightMode::Master => {
                crate::tui::ui::components::master_list_panel::render_master_list_panel(
                    f,
                    content_area,
                    app,
                )
            }
            _ => crate::tui::ui::components::slave_listen_panel::render_slave_listen_panel(
                f,
                content_area,
                app,
            ),
        },
        2 => render_pull_log(f, content_area, app),
        _ => render_pull_config(f, content_area, app),
    }
}

fn render_pull_config(f: &mut Frame, area: Rect, app: &Status) {
    // delegate to shared component implementation
    crate::tui::ui::components::config_panel::render_config_panel(f, area, app, None);
}

// registers rendering is delegated directly to components (master_list_panel/slave_listen_panel)

fn render_pull_log(f: &mut Frame, area: Rect, _app: &Status) {
    // delegate to shared component implementation
    crate::tui::ui::components::log_panel::render_log_panel(f, area, _app);
}

/// Handle key events when pull page is active. Return true if the event is consumed.
pub fn handle_subpage_key(
    key: crossterm::event::KeyEvent,
    app: &mut crate::protocol::status::Status,
) -> bool {
    use crossterm::event::KeyCode as KC;
    // If editing, let the global editing handler take care of chars/backspace/enter
    if let Some(form) = app.subpage_form.as_ref() {
        if form.editing {
            return true;
        }
    }
    // Tab/arrow navigation is handled by the parent input handler; fall through to allow
    // parent to process Tab/BackTab/Right/Left/Up/Down when appropriate.

    match key.code {
        // Allow local Tab-based subpage tab switching (3 tabs: config, list/log)
        KC::Tab => {
            // 3 tabs in this page
            app.subpage_tab_index = (app.subpage_tab_index + 1) % 3;
            return true;
        }
        KC::BackTab => {
            let total = 3usize;
            if app.subpage_tab_index == 0 {
                app.subpage_tab_index = total - 1;
            } else {
                app.subpage_tab_index -= 1;
            }
            return true;
        }
        KC::Enter => {
            if let Some(form) = app.subpage_form.as_mut() {
                form.editing = !form.editing;
                if form.editing {
                    form.input_buffer.clear();
                } else {
                    form.editing_field = None;
                    form.input_buffer.clear();
                }
            }
            return true;
        }
        KC::Char('e') => {
            if let Some(form) = app.subpage_form.as_mut() {
                form.editing = !form.editing;
                if form.editing {
                    form.input_buffer.clear();
                }
            }
            return true;
        }
        KC::Char('n') => {
            // add register
            if app.subpage_form.is_none() {
                app.init_subpage_form();
            }
            if let Some(form) = app.subpage_form.as_mut() {
                form.registers.push(crate::protocol::status::RegisterEntry {
                    slave_id: 1,
                    mode: 1,
                    address: 0,
                    length: 1,
                });
            }
            return true;
        }
        KC::Char('d') => {
            if let Some(form) = app.subpage_form.as_mut() {
                form.registers.pop();
            }
            return true;
        }
        // form cursor movement delegated to parent (MoveNext/MovePrev) when subpage is active
        _ => {}
    }
    false
}
