use ratatui::{prelude::*, text::Line};

use crate::{i18n::lang, protocol::status::Status};

/// Render a master list panel (distinct from registers_panel) — uses i18n strings.
pub fn render_master_list_panel(f: &mut Frame, area: Rect, app: &Status) {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(lang().label_master_list.as_str()));
    if let Some(form) = app.subpage_form.as_ref() {
        for (i, r) in form.registers.iter().enumerate() {
            let line_text = format!(
                "{}. slave={} mode={} addr={} len={}",
                i + 1,
                r.slave_id,
                r.mode,
                r.address,
                r.length
            );
            lines.push(Line::from(line_text));
        }
    } else {
        lines.push(Line::from(lang().details_placeholder.as_str()));
    }
    crate::tui::ui::components::render_boxed_paragraph(f, area, lines, None);
}
