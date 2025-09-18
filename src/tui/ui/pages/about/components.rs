use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

use crate::{i18n::lang, tui::ui::components::kv_pairs_to_lines};

#[derive(Default, Clone)]
pub struct RepoManifest {
    pub full_name: String,
    pub version: String,
    pub authors: String,
    pub repo: String,
    pub license: String,
    pub deps: Vec<(String, String)>,
    pub license_map: HashMap<String, String>,
}

static ABOUT_CACHE: OnceLock<Arc<Mutex<RepoManifest>>> = OnceLock::new();

// Initialize ABOUT_CACHE from the generated TOML snapshot at compile time.
pub(crate) fn init_about_cache() -> Arc<Mutex<RepoManifest>> {
    if let Some(v) = ABOUT_CACHE.get() {
        return v.clone();
    }

    // Embedded file produced by build.rs. Path adjusted for files under `about/`.
    const ABOUT_TOML: &str = include_str!("../../../../../res/about_cache.toml");

    let mut cache = RepoManifest::default();

    match toml::from_str::<toml::Value>(ABOUT_TOML) {
        Ok(val) => {
            // package section
            if let Some(pkg) = val.get("package") {
                if let Some(n) = pkg.get("name").and_then(|v| v.as_str()) {
                    cache.full_name = n.to_string();
                }
                if let Some(ver) = pkg.get("version").and_then(|v| v.as_str()) {
                    cache.version = ver.to_string();
                }
                if let Some(a) = pkg.get("authors").and_then(|v| v.as_array()) {
                    let auth_str = a
                        .iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    cache.authors = auth_str;
                }
                if let Some(repo) = pkg.get("repository").and_then(|v| v.as_str()) {
                    cache.repo = repo.to_string();
                }
                if let Some(lic) = pkg.get("license").and_then(|v| v.as_str()) {
                    cache.license = lic.to_string();
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
        }
        Err(err) => {
            log::error!("Failed to parse about_cache.toml: {err}");
        }
    }

    let arc = Arc::new(Mutex::new(cache));
    if ABOUT_CACHE.set(arc.clone()).is_err() {
        log::warn!("Failed to set ABOUT_CACHE, use default value instead");
        return Arc::new(Mutex::new(RepoManifest::default()));
    }
    arc
}

/// Render about content on right panel. Reads Cargo.toml at repo root and shows package and deps.
/// Render the about details (label/value pairs) into lines. This can be used both for
/// the entry preview and the full about subpage.
pub fn render_about_page_manifest_lines(app_snapshot: RepoManifest) -> Vec<Line<'static>> {
    let mut out: Vec<Line> = Vec::new();

    out.push(Line::from(lang().about.welcome.clone()));
    out.push(Line::from(Span::raw("")));

    let mut base_pairs: Vec<(String, String)> = Vec::new();

    base_pairs.push((lang().about.version.clone(), app_snapshot.version.clone()));
    base_pairs.push((lang().about.authors.clone(), app_snapshot.authors.clone()));
    base_pairs.push((lang().about.repository.clone(), app_snapshot.repo.clone()));
    base_pairs.push((lang().about.license.clone(), app_snapshot.license.clone()));

    // Render base pairs first
    let mut base_kv_lines = kv_pairs_to_lines(&base_pairs, 5);
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

use anyhow::Result;

use crate::protocol::status::{types, write_status};

/// Scroll the About page view offset up by `amount` (saturating at 0).
pub fn about_scroll_up(amount: usize) -> Result<()> {
    write_status(|s| {
        if let types::Page::About { view_offset } = &mut s.page {
            if *view_offset > 0 {
                *view_offset = view_offset.saturating_sub(amount);
            }
        }
        Ok(())
    })?;
    Ok(())
}

/// Scroll the About page view offset down by `amount`.
pub fn about_scroll_down(amount: usize) -> Result<()> {
    write_status(|s| {
        if let types::Page::About { view_offset } = &mut s.page {
            *view_offset = view_offset.saturating_add(amount);
        }
        Ok(())
    })?;
    Ok(())
}
