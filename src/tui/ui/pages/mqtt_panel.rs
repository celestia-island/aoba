use ratatui::{prelude::*, widgets::Paragraph};

use crate::{
    i18n::lang,
    protocol::status::{AppMode, Status},
};

pub fn render_mqtt_panel(f: &mut Frame, area: Rect, app: &Status) {
    let mode = app.page.app_mode;
    let text = match mode {
        AppMode::Mqtt => lang().protocol.mqtt.panel_placeholder.as_str(),
        _ => lang().protocol.mqtt.panel_not_current.as_str(),
    };
    let p = Paragraph::new(text).wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(p, area);
}

pub fn page_bottom_hints(_app: &Status) -> Vec<String> {
    // MQTT panel currently has no interactive editing; return empty hints
    Vec::new()
}

pub fn map_key(
    _key: crossterm::event::KeyEvent,
    _app: &Status,
) -> Option<crate::tui::input::Action> {
    None
}

use crate::tui::utils::bus::Bus;
use crossterm::event::KeyEvent;

pub fn handle_subpage_key(_key: KeyEvent, _app: &mut Status, _bus: &Bus) -> bool {
    false
}
