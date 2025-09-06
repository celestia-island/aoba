use serde_json::Value as JsonValue;
use std::{
    collections::HashMap,
    process::Command,
    sync::{Arc, Mutex, OnceLock},
};

use ratatui::{
    prelude::*,
    text::Line,
    text::Span,
    widgets::{Block, Paragraph},
};

use crate::{
    i18n::lang,
    protocol::status::Status,
    tui::ui::components::{kv_pairs_to_lines, render_boxed_paragraph},
};
use ratatui::style::{Color, Style};
use unicode_width::UnicodeWidthStr;

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

// Return bottom hints for about page (same as entry, but no extras)
pub fn page_bottom_hints(_app: &Status) -> Vec<String> {
    Vec::new()
}

pub fn map_key(
    _key: crossterm::event::KeyEvent,
    _app: &Status,
) -> Option<crate::tui::input::Action> {
    None
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
                format!("{} ({})", decl, license),
                Style::default().fg(Color::DarkGray),
            );
            out.push(Line::from(vec![name_span, spacer, license_span]));
        }
    }

    out
}

pub fn render_about(f: &mut Frame, area: Rect, app: &Status) {
    let block = Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .title(Span::raw(format!(" {}", lang().index.title)));
    // Use a global cache protected by a Mutex so background thread can populate it once

    // initialize cache holder and spawn loader once
    if ABOUT_CACHE.get().is_none() {
        let holder = Arc::new(Mutex::new(AboutCache {
            ready: false,
            ..Default::default()
        }));
        let _ = ABOUT_CACHE.set(holder.clone());
        std::thread::spawn(move || {
            let mut cache = AboutCache::default();
            // read Cargo.toml
            match std::fs::read_to_string("Cargo.toml") {
                Ok(s) => {
                    if let Ok(v) = toml::from_str::<toml::Value>(&s) {
                        if let Some(pkg) = v.get("package") {
                            cache.full_name = pkg
                                .get("name")
                                .and_then(|x| x.as_str())
                                .map(|s| s.to_string());
                            cache.version = pkg
                                .get("version")
                                .and_then(|x| x.as_str())
                                .map(|s| s.to_string());
                            cache.authors =
                                pkg.get("authors").and_then(|x| x.as_array()).map(|arr| {
                                    arr.iter()
                                        .filter_map(|a| a.as_str())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                });
                            cache.repo = pkg
                                .get("repository")
                                .and_then(|x| x.as_str())
                                .map(|s| s.to_string());
                            cache.license = pkg
                                .get("license")
                                .and_then(|x| x.as_str())
                                .map(|s| s.to_string());
                        }
                        if let Some(deps) = v.get("dependencies") {
                            if let Some(table) = deps.as_table() {
                                for (k, val) in table.iter().take(200) {
                                    let decl = if val.is_str() {
                                        val.as_str().unwrap_or("").to_string()
                                    } else if val.is_table() {
                                        val.get("version")
                                            .and_then(|x| x.as_str())
                                            .unwrap_or("")
                                            .to_string()
                                    } else {
                                        "".to_string()
                                    };
                                    cache.deps.push((k.clone(), decl));
                                }
                            }
                        }
                    } else {
                        cache.err = Some("Failed to parse Cargo.toml".to_string());
                    }
                }
                Err(e) => cache.err = Some(format!("Could not read Cargo.toml: {}", e)),
            }
            // try to read precomputed cache from res/about_cache.toml generated at build time
            if let Ok(s) = std::fs::read_to_string("res/about_cache.toml") {
                if let Ok(tv) = toml::from_str::<toml::Value>(&s) {
                    if let Some(pkg) = tv.get("package") {
                        cache.full_name = pkg
                            .get("name")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string());
                        cache.version = pkg
                            .get("version")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string());
                        cache.authors = pkg.get("authors").and_then(|x| x.as_array()).map(|arr| {
                            arr.iter()
                                .filter_map(|a| a.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        });
                        cache.repo = pkg
                            .get("repository")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string());
                        cache.license = pkg
                            .get("license")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string());
                    }
                    if let Some(deps) = tv.get("dependencies").and_then(|d| d.as_array()) {
                        for d in deps.iter() {
                            if let Some(n) = d.get("name").and_then(|x| x.as_str()) {
                                let v = d.get("version").and_then(|x| x.as_str()).unwrap_or("");
                                cache.deps.push((n.to_string(), v.to_string()));
                            }
                        }
                    }
                    if let Some(map) = tv.get("license_map").and_then(|m| m.as_table()) {
                        for (k, v) in map.iter() {
                            if let Some(sv) = v.as_str() {
                                cache.license_map.insert(k.clone(), sv.to_string());
                            }
                        }
                    }
                }
            } else {
                // cargo metadata for license map as fallback (non-blocking fallback will fill later)
                match Command::new("cargo")
                    .args(&["metadata", "--format-version", "1"])
                    .output()
                {
                    Ok(out) => {
                        if out.status.success() {
                            if let Ok(json) = serde_json::from_slice::<JsonValue>(&out.stdout) {
                                if let Some(pkgs) = json.get("packages").and_then(|p| p.as_array())
                                {
                                    for p in pkgs.iter() {
                                        if let Some(n) = p.get("name").and_then(|x| x.as_str()) {
                                            let lic = p
                                                .get("license")
                                                .and_then(|x| x.as_str())
                                                .unwrap_or("");
                                            let ver = p
                                                .get("version")
                                                .and_then(|x| x.as_str())
                                                .unwrap_or("");
                                            cache
                                                .license_map
                                                .insert(format!("{}:{}", n, ver), lic.to_string());
                                            cache
                                                .license_map
                                                .insert(n.to_string(), lic.to_string());
                                        }
                                    }
                                }
                            }
                        } else {
                            cache.err = Some("cargo metadata returned non-zero".to_string());
                        }
                    }
                    Err(e) => cache.err = Some(format!("Failed to execute cargo metadata: {}", e)),
                }
            }

            cache.ready = true;
            if let Some(h) = ABOUT_CACHE.get() {
                if let Ok(mut g) = h.lock() {
                    *g = cache;
                }
            }
        });
    }

    // Build lines from cache snapshot into a full list, then perform windowing using app.about_view_offset
    let mut full_lines: Vec<Line> = Vec::new();
    if let Some(h) = ABOUT_CACHE.get() {
        if let Ok(g) = h.lock() {
            // Use helper to build details lines from an owned snapshot to avoid holding the lock
            full_lines = render_about_details(g.clone());
            if let Some(e) = &g.err {
                full_lines.push(Line::from(format!("Note: {}", e)));
            }
        } else {
            full_lines.push(Line::from("Failed to lock about cache"));
        }
    } else {
        full_lines.push(Line::from("Initializing about cache..."));
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

    // read current offset from global app state (use app.about_view_offset)
    let first_visible = std::cmp::min(max_start, app.about_view_offset);
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
        let ratio = if denom > 0.0 {
            first_visible as f32 / denom
        } else {
            0.0
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
