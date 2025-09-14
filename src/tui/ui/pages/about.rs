use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock, RwLock},
};

use ratatui::{
    prelude::*,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    i18n::lang,
    protocol::status::types::{self, Status},
    tui::ui::components::{kv_pairs_to_lines, render_boxed_paragraph},
    tui::utils::bus::Bus,
};

#[derive(Default, Clone)]
pub(crate) struct AboutCache {
    full_name: Option<String>,
    version: Option<String>,
    authors: Option<String>,
    repo: Option<String>,
    license: Option<String>,
    deps: Vec<(String, String)>,
    license_map: HashMap<String, String>,
    ready: bool,
    err: Option<String>,
}

static ABOUT_CACHE: OnceLock<Arc<Mutex<AboutCache>>> = OnceLock::new();

// Initialize ABOUT_CACHE from the generated TOML snapshot at compile time.
fn init_about_cache() -> Arc<Mutex<AboutCache>> {
    if let Some(v) = ABOUT_CACHE.get() {
        return v.clone();
    }

    // Embedded file produced by build.rs
    const ABOUT_TOML: &str = include_str!("../../../../res/about_cache.toml");

    let mut cache = AboutCache::default();

    match toml::from_str::<toml::Value>(ABOUT_TOML) {
        Ok(val) => {
            // package section
            if let Some(pkg) = val.get("package") {
                if let Some(n) = pkg.get("name").and_then(|v| v.as_str()) {
                    cache.full_name = Some(n.to_string());
                }
                if let Some(ver) = pkg.get("version").and_then(|v| v.as_str()) {
                    cache.version = Some(ver.to_string());
                }
                if let Some(a) = pkg.get("authors").and_then(|v| v.as_array()) {
                    let auth_str = a
                        .iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    if !auth_str.is_empty() {
                        cache.authors = Some(auth_str);
                    }
                }
                if let Some(repo) = pkg.get("repository").and_then(|v| v.as_str()) {
                    cache.repo = Some(repo.to_string());
                }
                if let Some(lic) = pkg.get("license").and_then(|v| v.as_str()) {
                    cache.license = Some(lic.to_string());
                }
            }

            // deps: support both forms that may be produced by build.rs
            // 1) a top-level table array named `deps` (legacy used in some builds)
            // 2) a table array named `dependencies` (Cargo-style emitted by cargo metadata)
            let mut added = Vec::new();
            if let Some(deps) = val.get("deps").and_then(|v| v.as_array()) {
                for dep in deps {
                    if let Some(dep_map) = dep.as_table() {
                        if let (Some(name), Some(version)) = (
                            dep_map.get("name").and_then(|v| v.as_str()),
                            dep_map.get("version").and_then(|v| v.as_str()),
                        ) {
                            cache.deps.push((name.to_string(), version.to_string()));
                            added.push(name.to_string());
                            if let Some(lic) = dep_map.get("license").and_then(|v| v.as_str()) {
                                cache.license_map.insert(name.to_string(), lic.to_string());
                            }
                        }
                    }
                }
            }

            // Cargo-style `[[dependencies]]` sections: iterate all entries
            if let Some(deps2) = val.get("dependencies").and_then(|v| v.as_array()) {
                for dep in deps2 {
                    if let Some(dep_map) = dep.as_table() {
                        if let (Some(name), Some(version)) = (
                            dep_map.get("name").and_then(|v| v.as_str()),
                            dep_map.get("version").and_then(|v| v.as_str()),
                        ) {
                            if !added.contains(&name.to_string()) {
                                cache.deps.push((name.to_string(), version.to_string()));
                                added.push(name.to_string());
                            }
                            if let Some(lic) = dep_map.get("license").and_then(|v| v.as_str()) {
                                cache.license_map.insert(name.to_string(), lic.to_string());
                            }
                        }
                    }
                }
            }

            // license_map: accept an explicit table mapping name -> license string
            if let Some(lmap) = val.get("license_map").and_then(|v| v.as_table()) {
                for (k, v) in lmap.iter() {
                    if let Some(s) = v.as_str() {
                        cache
                            .license_map
                            .entry(k.clone())
                            .or_insert_with(|| s.to_string());
                    }
                }
            }
            cache.ready = true;
        }
        Err(e) => {
            cache.err = Some(format!("Error parsing about_cache.toml: {e}"));
            cache.ready = true;
        }
    }

    let arc = Arc::new(Mutex::new(cache));
    let _ = ABOUT_CACHE.set(arc.clone());
    arc
}

// Return bottom hints for about page (same as entry, but no extras)
pub fn page_bottom_hints(_app: &Status, _snap: &types::ui::AboutStatus) -> Vec<String> {
    Vec::new()
}

pub fn map_key(
    _key: crossterm::event::KeyEvent,
    _app: &Status,
    _snap: &types::ui::AboutStatus,
) -> Option<crate::tui::input::Action> {
    None
}

/// Handle input for about page. Sends navigation commands via UiToCore.
pub fn handle_input(
    key: crossterm::event::KeyEvent,
    _app: &Status,
    bus: &Bus,
    app_arc: &Arc<RwLock<types::Status>>,
    _snap: &types::ui::AboutStatus,
) -> bool {
    use crossterm::event::KeyCode as KC;

    // Build the full lines snapshot to determine bounds for scrolling.
    let mut full_lines: Vec<ratatui::text::Line> = Vec::new();
    let h = init_about_cache();
    if let Ok(g) = h.lock() {
        full_lines = render_about_details(g.clone());
        if let Some(e) = &g.err {
            full_lines.push(ratatui::text::Line::from(format!("Note: {e}")));
        }
    }

    let total = full_lines.len();
    // inner height inside borders (same calculation as render)
    // We can't access area here, so assume caller will clamp; we'll operate on relative moves.

    match key.code {
        KC::Up | KC::Char('k') => {
            // move up by one line
            let _ = crate::protocol::status::write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    if *view_offset > 0 {
                        *view_offset = view_offset.saturating_sub(1);
                    }
                }
                Ok(())
            });
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        KC::Down | KC::Char('j') => {
            // move down by one line
            let _ = crate::protocol::status::write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    // allow increment; render will clamp based on available height
                    *view_offset = view_offset.saturating_add(1);
                }
                Ok(())
            });
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        KC::PageUp => {
            // jump up by a page: we approximate page size by 10 lines (render will clamp)
            let page = 10usize;
            let _ = crate::protocol::status::write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    *view_offset = view_offset.saturating_sub(page);
                }
                Ok(())
            });
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        KC::PageDown => {
            let page = 10usize;
            let _ = crate::protocol::status::write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    *view_offset = view_offset.saturating_add(page);
                }
                Ok(())
            });
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        KC::Home => {
            let _ = crate::protocol::status::write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    *view_offset = 0;
                }
                Ok(())
            });
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        KC::End => {
            // set to an arbitrarily large offset; render will clamp to max_start
            let _ = crate::protocol::status::write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    *view_offset = total.saturating_sub(1);
                }
                Ok(())
            });
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        _ => false,
    }
}

/// Handle mouse events for About page (scroll wheel). Return true when consumed.
pub fn handle_mouse(
    me: crossterm::event::MouseEvent,
    _app: &Status,
    bus: &Bus,
    app_arc: &Arc<RwLock<types::Status>>,
    _snap: &types::ui::AboutStatus,
) -> bool {
    use crossterm::event::MouseEventKind as MEK;

    // Only handle scroll events here
    match me.kind {
        MEK::ScrollUp => {
            // move up one line
            let _ = crate::protocol::status::write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    if *view_offset > 0 {
                        *view_offset = view_offset.saturating_sub(1);
                    }
                }
                Ok(())
            });
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        MEK::ScrollDown => {
            // move down one line
            let _ = crate::protocol::status::write_status(app_arc, |s| {
                if let types::Page::About { view_offset } = &mut s.page {
                    *view_offset = view_offset.saturating_add(1);
                }
                Ok(())
            });
            let _ = bus.ui_tx.send(crate::tui::utils::bus::UiToCore::Refresh);
            true
        }
        _ => false,
    }
}

/// Render about content on right panel. Reads Cargo.toml at repo root and shows package and deps.
/// Render the about details (label/value pairs) into lines. This can be used both for
/// the entry preview and the full about subpage.
pub(crate) fn render_about_details(app_snapshot: AboutCache) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();
    if !app_snapshot.ready {
        lines.push(Line::from("Loading about information..."));
        return lines;
    }
    // Welcome paragraph (localized)
    let mut out: Vec<Line> = Vec::new();
    out.push(Line::from(lang().about.welcome.clone()));
    // Add a blank line after welcome paragraph for spacing
    out.push(Line::from(Span::raw("")));

    // Base info labels use i18n (skip Name; welcome covers it)
    let mut base_pairs: Vec<(String, String, Option<ratatui::style::Style>)> = Vec::new();
    if let Some(ver) = &app_snapshot.version {
        base_pairs.push((lang().about.version.clone(), ver.clone(), None));
    }
    if let Some(auth) = &app_snapshot.authors {
        base_pairs.push((lang().about.authors.clone(), auth.clone(), None));
    }
    if let Some(repo) = &app_snapshot.repo {
        base_pairs.push((lang().about.repository.clone(), repo.clone(), None));
    }
    if let Some(lic) = &app_snapshot.license {
        base_pairs.push((lang().about.license.clone(), lic.clone(), None));
    }

    // Render base pairs first
    let mut base_kv_lines = kv_pairs_to_lines(&base_pairs, "  ", 5);
    out.append(&mut base_kv_lines);

    // Then render dependencies header + dependency items (if any)
    if !app_snapshot.deps.is_empty() {
        // blank separator
        out.push(Line::from(Span::raw("")));
        // Bold dependencies header (use localized text)
        out.push(Line::from(Span::styled(
            lang().about.dependencies_license_list.clone(),
            ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::BOLD),
        )));

        // Render each dependency as: name (normal) + gap + decl (license info) — do not bold the name
        // compute max name width for alignment
        let max_name_w = app_snapshot
            .deps
            .iter()
            .map(|(n, _)| UnicodeWidthStr::width(n.as_str()))
            .max()
            .unwrap_or(0usize);
        for (name, decl) in app_snapshot.deps.iter() {
            let license = app_snapshot
                .license_map
                .get(name)
                .map(|s| s.as_str())
                .unwrap_or("-");
            let name_w = UnicodeWidthStr::width(name.as_str());
            let pad = max_name_w.saturating_sub(name_w);
            let padded_name = format!("  {}{}", name, " ".repeat(pad));
            let name_span = Span::raw(padded_name);
            let spacer = Span::raw("  ");
            let license_span = Span::styled(
                format!("{decl} ({license})"),
                Style::default().fg(Color::DarkGray),
            );
            out.push(Line::from(vec![name_span, spacer, license_span]));
        }
    }

    out
}

/// Render the about page. Only reads from Status, does not mutate.
pub fn render(f: &mut Frame, area: Rect, _app: &Status, snap: &types::ui::AboutStatus) {
    let block = Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .title(Span::raw(format!(" {}", lang().index.title)));

    // Build lines from cache snapshot into a full list, then perform windowing using app.ports.about_view_offset
    let mut full_lines: Vec<Line> = Vec::new();
    // ensure ABOUT_CACHE initialized
    let h = init_about_cache();
    if let Ok(g) = h.lock() {
        // Use helper to build details lines from an owned snapshot to avoid holding the lock
        full_lines = render_about_details(g.clone());
        if let Some(e) = &g.err {
            full_lines.push(Line::from(format!("Note: {e}")));
        }
    } else {
        full_lines.push(Line::from("Failed to lock about cache"));
    }

    // pagination / scrolling (reuse logic from log_panel/modbus_panel)
    let inner_height = area.height.saturating_sub(2) as usize; // inside borders
    let total = full_lines.len();
    if inner_height == 0 {
        // nothing to render
        let para = Paragraph::new(full_lines).block(block);
        f.render_widget(para, area);
        return;
    }

    // clamp about_view_offset to valid range
    if total == 0 {
        // render empty block
        let para = Paragraph::new(full_lines).block(block);
        f.render_widget(para, area);
        return;
    }

    let max_start = total.saturating_sub(inner_height);
    if area.width == 0 {
        return;
    }

    // read current offset from global app state
    let first_visible = std::cmp::min(max_start, snap.view_offset);
    let end = std::cmp::min(total, first_visible + inner_height);
    let windowed: Vec<Line> = full_lines[first_visible..end].to_vec();

    // Render paragraph content (leave room for scrollbar if needed)
    let content_rect = Rect::new(area.x, area.y, area.width.saturating_sub(1), area.height);
    render_boxed_paragraph(f, content_rect, windowed, None);

    // draw scrollbar if needed (inline implementation similar to modbus_panel::render_scroll_bar)
    if total > inner_height {
        let bar_x = area.x + area.width.saturating_sub(1);
        let bar_y = area.y + 1;
        let bar_h = area.height.saturating_sub(2);
        let denom = (total.saturating_sub(inner_height)) as f32;
        let ratio = if denom > 0. {
            first_visible as f32 / denom
        } else {
            0.
        };
        let thumb = bar_y + ((ratio * (bar_h.saturating_sub(1) as f32)).round() as u16);
        for i in 0..bar_h {
            let ch = if bar_y + i == thumb { '█' } else { '│' };
            let p = ratatui::widgets::Paragraph::new(ch.to_string())
                .style(ratatui::style::Style::default().fg(ratatui::style::Color::DarkGray));
            f.render_widget(p, Rect::new(bar_x, bar_y + i, 1, 1));
        }
    }
}
