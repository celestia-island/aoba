use ratatui::{prelude::*, widgets::Paragraph};

use crate::{
    i18n::lang,
    protocol::status::{AppMode, Status},
};

pub fn render_mqtt_panel(f: &mut Frame, area: Rect, app: &Status) {
    let text = match app.ui.app_mode {
        AppMode::Mqtt => lang().protocol.mqtt.panel_placeholder.as_str(),
        AppMode::Modbus => lang().protocol.mqtt.panel_not_current.as_str(),
    };
    let p = Paragraph::new(text).wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(p, area);
}
