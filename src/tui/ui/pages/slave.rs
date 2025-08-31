use crossterm::event::KeyEvent;
use ratatui::{
    prelude::*,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Tabs},
};

use crate::{
    i18n::lang,
    protocol::status::Status,
    tui::{edit, input::Action},
};

/// UI for configuring Modbus master settings for the selected port.
pub fn render_slave(f: &mut Frame, area: Rect, app: &mut Status) {
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
        Span::raw(format!(" - {}", lang().tab_master.as_str())),
    ]);

    let middle_tab = match app.right_mode {
        crate::protocol::status::RightMode::Master => lang().label_master_list.as_str(),
        crate::protocol::status::RightMode::SlaveStack => lang().label_slave_listen.as_str(),
        crate::protocol::status::RightMode::Listen => lang().label_slave_listen.as_str(),
    };
    let tabs = vec!["通信配置", middle_tab, "通信日志"];
    let tab_index = app.subpage_tab_index.min(tabs.len().saturating_sub(1));

    // Use a single-line header so tabs sit directly above content (no extra empty row)
    let [header_area, content_area] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1),
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
        0 => render_slave_config(f, content_area, app),
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
        2 => render_slave_log(f, content_area, app),
        _ => render_slave_config(f, content_area, app),
    }
}

fn render_slave_config(f: &mut Frame, area: Rect, app: &mut Status) {
    // delegate to shared component implementation
    crate::tui::ui::components::config_panel::render_config_panel(f, area, app, None);
}

// registers rendering is delegated directly to components (master_list_panel/slave_listen_panel)

fn render_slave_log(f: &mut Frame, area: Rect, _app: &mut Status) {
    // delegate to shared component implementation
    crate::tui::ui::components::log_panel::render_log_panel(f, area, _app);
}

/// Handle key events when slave page is active. Return true if the event is consumed.
pub fn handle_subpage_key(
    key: crossterm::event::KeyEvent,
    app: &mut crate::protocol::status::Status,
) -> bool {
    use crossterm::event::KeyCode as KC;
    // If currently editing a field, consume keys here so parent won't process navigation.
    if let Some(form) = app.subpage_form.as_ref() {
        if form.editing {
            return true;
        }
    }

    // Tab/arrow navigation is handled by the parent input handler; fall through to allow
    // parent to process Tab/BackTab/Right/Left/Up/Down when appropriate.

    match key.code {
        // 三个子标签切换
        KC::Tab => {
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
        // 进入/退出编辑
        KC::Enter => {
            if let Some(form) = app.subpage_form.as_mut() {
                if !form.editing {
                    edit::begin_edit(form);
                } else {
                    edit::end_edit(form);
                }
            }
            return true;
        }
        KC::Char('e') => {
            if let Some(form) = app.subpage_form.as_mut() {
                if !form.editing {
                    edit::begin_edit(form);
                } else {
                    edit::end_edit(form);
                }
            }
            return true;
        }
        // 光标上移
        KC::Up | KC::Char('k') => {
            if app.subpage_tab_index == 2 {
                return false;
            } // 让父级处理日志滚动
            if let Some(form) = app.subpage_form.as_mut() {
                let total = 4usize.saturating_add(form.registers.len());
                if total > 0 {
                    form.cursor = if form.cursor == 0 {
                        total - 1
                    } else {
                        form.cursor - 1
                    };
                }
            }
            return true;
        }
        // 光标下移
        KC::Down | KC::Char('j') => {
            if app.subpage_tab_index == 2 {
                return false;
            }
            if let Some(form) = app.subpage_form.as_mut() {
                let total = 4usize.saturating_add(form.registers.len());
                if total > 0 {
                    form.cursor = (form.cursor + 1) % total;
                }
            }
            return true;
        }
        // 新增寄存器（与 map_key 中 AddRegister 对应）
        KC::Char('n') => {
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
        _ => {}
    }
    false
}

/// Provide bottom hints when this page is active as a subpage.
pub fn page_bottom_hints(app: &Status) -> Vec<String> {
    let mut hints: Vec<String> = Vec::new();
    if let Some(form) = &app.subpage_form {
        if form.editing {
            if !form.edit_confirmed {
                hints.push(lang().press_enter_select.as_str().to_string());
                hints.push(lang().press_esc_cancel.as_str().to_string());
            } else {
                hints.push(lang().press_enter_submit.as_str().to_string());
                hints.push(lang().press_esc_cancel.as_str().to_string());
            }
            return hints;
        }
    }
    // If current tab is the log tab, show log-input related hints
    // If current tab is the configuration tab, show config-specific hints
    if app.subpage_tab_index == 0 {
        // show Enter to open editor and movement hint (Up/Down or k/j)
        hints.push(lang().press_enter_confirm_edit.as_str().to_string());
        hints.push(lang().hint_move_vertical.as_str().to_string());
        return hints;
    }

    // If current tab is the middle list tab, do not show page-specific hints (leave blank)
    if app.subpage_tab_index == 1 {
        return hints;
    }

    if app.subpage_tab_index == 2 {
        if app.input_editing {
            hints.push(lang().press_enter_submit.as_str().to_string());
            hints.push(lang().press_esc_cancel.as_str().to_string());
        } else {
            // show short kv-style hints (按 i 编辑, 按 m 切换模式) in the bottom bar
            hints.push(crate::tui::ui::bottom::format_kv_hint(
                "Enter/i",
                lang().hint_input_edit_short.as_str(),
            ));
            hints.push(crate::tui::ui::bottom::format_kv_hint(
                "m",
                lang().hint_input_mode_short.as_str(),
            ));
            // show follow/track latest toggle (p) -- show the action (inverse of current state)
            let action_label = if app.log_auto_scroll {
                lang().hint_follow_off.as_str()
            } else {
                lang().hint_follow_on.as_str()
            };
            hints.push(crate::tui::ui::bottom::format_kv_hint("p", action_label));
            // (mode description moved into the input placeholder; skip duplicate)
        }
        return hints;
    }

    hints.push(lang().hint_back_list.as_str().to_string());
    hints.push(lang().hint_switch_tab.as_str().to_string());
    hints
}

/// Page-level key mapping: allow slave page to map keys to Actions (optional).
pub fn map_key(key: KeyEvent, _app: &Status) -> Option<Action> {
    use crossterm::event::KeyCode as KC;
    match key.code {
        KC::Tab => Some(Action::SwitchNext),
        KC::BackTab => Some(Action::SwitchPrev),
        KC::Enter => Some(Action::EditToggle),
        KC::Char('e') => Some(Action::EditToggle),
        KC::Char('n') => Some(Action::AddRegister),
        KC::Up | KC::Char('k') => Some(Action::MovePrev),
        KC::Down | KC::Char('j') => Some(Action::MoveNext),
        _ => None,
    }
}
