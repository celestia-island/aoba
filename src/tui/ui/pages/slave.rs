use crossterm::event::KeyEvent;
use ratatui::{
    prelude::*,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Tabs},
};

use crate::{i18n::lang, protocol::status::Status, tui::input::Action};

/// UI for configuring Modbus master settings for the selected port.
pub fn render_slave(f: &mut Frame, area: Rect, app: &Status) {
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

fn render_slave_config(f: &mut Frame, area: Rect, app: &Status) {
    // delegate to shared component implementation
    crate::tui::ui::components::config_panel::render_config_panel(f, area, app, None);
}

// registers rendering is delegated directly to components (master_list_panel/slave_listen_panel)

fn render_slave_log(f: &mut Frame, area: Rect, _app: &Status) {
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
        KC::Enter => {
            if let Some(form) = app.subpage_form.as_mut() {
                // Toggle editing mode for the currently selected field.
                if !form.editing {
                    form.editing = true;
                    // choose field based on cursor: 0=baud,1=parity,2=databits,3=stopbits,>=4 -> register index
                    match form.cursor {
                        0 => form.editing_field = Some(crate::protocol::status::EditingField::Baud),
                        1 => {
                            form.editing_field = Some(crate::protocol::status::EditingField::Parity)
                        }
                        2 => {
                            form.editing_field =
                                Some(crate::protocol::status::EditingField::DataBits)
                        }
                        3 => {
                            form.editing_field =
                                Some(crate::protocol::status::EditingField::StopBits)
                        }
                        n => {
                            let ridx = n.saturating_sub(4);
                            form.editing_field =
                                Some(crate::protocol::status::EditingField::RegisterField {
                                    idx: ridx,
                                    field: crate::protocol::status::RegisterField::SlaveId,
                                });
                        }
                    }
                    form.input_buffer.clear();
                } else {
                    // If already editing, let the top-level input handler finish/commit the edit.
                }
            }
            return true;
        }
        KC::Up | KC::Char('k') => {
            if let Some(form) = app.subpage_form.as_mut() {
                let total = 4usize.saturating_add(form.registers.len());
                if total > 0 {
                    if form.cursor == 0 {
                        form.cursor = total - 1;
                    } else {
                        form.cursor -= 1;
                    }
                }
            }
            return true;
        }
        KC::Down | KC::Char('j') => {
            if let Some(form) = app.subpage_form.as_mut() {
                let total = 4usize.saturating_add(form.registers.len());
                if total > 0 {
                    form.cursor = (form.cursor + 1) % total;
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
