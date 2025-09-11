use ratatui::{prelude::*, widgets::Paragraph};

use crate::{
    i18n::lang,
    protocol::status::{ui as ui_accessors, AppMode, Status},
};

pub fn render_mqtt_panel(f: &mut Frame, area: Rect, app: &Status) {
    let mode = ui_accessors::ui_app_mode_get(app);
    let text = match mode {
        AppMode::Mqtt => lang().protocol.mqtt.panel_placeholder.as_str(),
        AppMode::Modbus => lang().protocol.mqtt.panel_not_current.as_str(),
    };
    let p = Paragraph::new(text).wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(p, area);
}
