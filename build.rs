use anyhow::Result;
use serde_json::Value as JsonValue;
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    process::Command,
};
use toml::value::{Table, Value as TomlValue};

use semver::Version;

fn main() -> Result<()> {
    // Build a TOML table as cache
    let mut out_tbl: Table = Table::new();

    // Try to read Cargo.toml package and dependencies
    if let Ok(s) = fs::read_to_string("Cargo.toml") {
        if let Ok(v) = toml::from_str::<toml::Value>(&s) {
            if let Some(pkg) = v.get("package") {
                if let Some(t) = pkg.as_table() {
                    let mut pj = Table::new();
                    if let Some(n) = t.get("name").and_then(|x| x.as_str()) {
                        pj.insert("name".to_string(), TomlValue::String(n.to_string()));
                    }
                    if let Some(vv) = t.get("version").and_then(|x| x.as_str()) {
                        pj.insert("version".to_string(), TomlValue::String(vv.to_string()));
                    }
                    if let Some(repo) = t.get("repository").and_then(|x| x.as_str()) {
                        pj.insert(
                            "repository".to_string(),
                            TomlValue::String(repo.to_string()),
                        );
                    }
                    if let Some(lic) = t.get("license").and_then(|x| x.as_str()) {
                        pj.insert("license".to_string(), TomlValue::String(lic.to_string()));
                    }
                    if let Some(auth) = t.get("authors").and_then(|x| x.as_array()) {
                        let arr = auth
                            .iter()
                            .filter_map(|a| a.as_str().map(|s| TomlValue::String(s.to_string())))
                            .collect::<Vec<_>>();
                        pj.insert("authors".to_string(), TomlValue::Array(arr));
                    }
                    out_tbl.insert("package".to_string(), TomlValue::Table(pj));
                }
            }
            if let Some(deps) = v.get("dependencies") {
                if let Some(table) = deps.as_table() {
                    let mut darr = Vec::new();
                    // collect first-level dependency package names (handle rename via `package` key)
                    let mut direct_dep_names: Vec<String> = Vec::new();
                    for (k, val) in table.iter().take(500) {
                        // Skip local / path dependencies (they are workspace crates and not relevant for external license summary)
                        if val.is_table() && val.get("path").is_some() {
                            // skip this dependency entirely
                            continue;
                        }

                        let ver = if val.is_str() {
                            val.as_str().unwrap_or("").to_string()
                        } else if val.is_table() {
                            val.get("version")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string()
                        } else {
                            "".to_string()
                        };
                        // determine the actual package name used in registry (if renamed, `package` field holds real name)
                        let actual_name = if val.is_table() {
                            val.get("package")
                                .and_then(|x| x.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or(k.to_string())
                        } else {
                            k.to_string()
                        };
                        direct_dep_names.push(actual_name.clone());

                        let mut dep_t = Table::new();
                        dep_t.insert("name".to_string(), TomlValue::String(actual_name.clone()));
                        if actual_name != k.as_str() {
                            dep_t.insert("alias".to_string(), TomlValue::String(k.to_string()));
                        }
                        dep_t.insert("version".to_string(), TomlValue::String(ver));
                        darr.push(TomlValue::Table(dep_t));
                    }
                    out_tbl.insert("dependencies".to_string(), TomlValue::Array(darr));
                    // store the direct dependency set in the table so later cargo-metadata handling can use it
                    let deps_name_arr = direct_dep_names
                        .into_iter()
                        .map(TomlValue::String)
                        .collect::<Vec<_>>();
                    out_tbl.insert(
                        "direct_dependency_names".to_string(),
                        TomlValue::Array(deps_name_arr),
                    );
                }
            }
        }
    }

    hydrate_package_metadata_from_env(&mut out_tbl);

    // Try cargo metadata for license map (metadata is JSON)
    if let Ok(o) = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .output()
    {
        if o.status.success() {
            if let Ok(jv) = serde_json::from_slice::<JsonValue>(&o.stdout) {
                if let Some(pkgs) = jv.get("packages").and_then(|p| p.as_array()) {
                    // build a set of direct dependency names from the earlier parsed Cargo.toml
                    let mut direct_set: HashSet<String> = HashSet::new();
                    if let Some(TomlValue::Array(arr)) = out_tbl.get("direct_dependency_names") {
                        for v in arr.iter() {
                            if let TomlValue::String(s) = v {
                                direct_set.insert(s.clone());
                            }
                        }
                        // remove the helper entry from out_tbl so it won't be part of final file
                        out_tbl.remove("direct_dependency_names");
                    }

                    // for each package name, keep only the entry with the highest semver version
                    let mut best_map: HashMap<String, (Version, String)> = HashMap::new();
                    for p in pkgs.iter() {
                        if let Some(n) = p.get("name").and_then(|x| x.as_str()) {
                            // only include first-level direct dependencies
                            if !direct_set.contains(n) {
                                continue;
                            }
                            let lic = p
                                .get("license")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string();
                            let ver_str = p.get("version").and_then(|x| x.as_str()).unwrap_or("");
                            if let Ok(ver) = Version::parse(ver_str) {
                                match best_map.get(n) {
                                    Some((existing_ver, _)) => {
                                        if &ver > existing_ver {
                                            best_map.insert(n.to_string(), (ver, lic));
                                        }
                                    }
                                    None => {
                                        best_map.insert(n.to_string(), (ver, lic));
                                    }
                                }
                            } else {
                                // if version cannot be parsed, prefer to insert if missing
                                best_map
                                    .entry(n.to_string())
                                    .or_insert((Version::new(0, 0, 0), lic));
                            }
                        }
                    }

                    if let Some(deps_arr) = out_tbl
                        .get_mut("dependencies")
                        .and_then(|v| v.as_array_mut())
                    {
                        for dep in deps_arr.iter_mut() {
                            if let TomlValue::Table(dep_tbl) = dep {
                                if let Some(dep_name) = dep_tbl.get("name").and_then(|x| x.as_str())
                                {
                                    if let Some((ver, _)) = best_map.get(dep_name) {
                                        dep_tbl.insert(
                                            "version".to_string(),
                                            TomlValue::String(format_version_for_about(ver)),
                                        );
                                    }
                                }
                            }
                        }
                    }

                    let mut map_tbl = Table::new();
                    for (name, (_ver, lic)) in best_map.into_iter() {
                        // only write a single key per package: `name` -> license
                        map_tbl.insert(name, TomlValue::String(lic));
                    }
                    out_tbl.insert("license_map".to_string(), TomlValue::Table(map_tbl));
                }
            }
        }
    }

    // write to res/about_cache.toml
    let toml_val = TomlValue::Table(out_tbl);
    if let Ok(content) = toml::to_string_pretty(&toml_val) {
        fs::create_dir_all("res")?;
        fs::write("res/about_cache.toml", content)?;
    }

    Ok(())
}

fn hydrate_package_metadata_from_env(out_tbl: &mut Table) {
    let pkg_tbl = match out_tbl.get_mut("package") {
        Some(TomlValue::Table(tbl)) => tbl,
        _ => {
            out_tbl.insert("package".to_string(), TomlValue::Table(Table::new()));
            match out_tbl.get_mut("package") {
                Some(TomlValue::Table(tbl)) => tbl,
                _ => return,
            }
        }
    };

    let name = env::var("CARGO_PKG_NAME").unwrap_or_default();
    set_string_field_if_missing(pkg_tbl, "name", &name);

    let version = env::var("CARGO_PKG_VERSION").unwrap_or_default();
    set_string_field_if_missing(pkg_tbl, "version", &version);

    let repository = env::var("CARGO_PKG_REPOSITORY").unwrap_or_default();
    set_string_field_if_missing(pkg_tbl, "repository", &repository);

    let license = env::var("CARGO_PKG_LICENSE").unwrap_or_default();
    set_string_field_if_missing(pkg_tbl, "license", &license);

    let authors_raw = env::var("CARGO_PKG_AUTHORS").unwrap_or_default();
    let authors: Vec<String> = authors_raw
        .split(':')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.to_string())
        .collect();

    if !authors.is_empty() {
        let should_set = match pkg_tbl.get("authors") {
            Some(TomlValue::Array(existing)) => existing
                .iter()
                .filter_map(|value| value.as_str())
                .all(|value| value.trim().is_empty()),
            Some(_) => true,
            None => true,
        };

        if should_set {
            let arr = authors
                .into_iter()
                .map(TomlValue::String)
                .collect::<Vec<_>>();
            pkg_tbl.insert("authors".to_string(), TomlValue::Array(arr));
        }
    }
}

fn set_string_field_if_missing(tbl: &mut Table, key: &str, value: &str) {
    if value.is_empty() {
        return;
    }

    let should_set = match tbl.get(key) {
        Some(TomlValue::String(existing)) => existing.trim().is_empty(),
        Some(_) => true,
        None => true,
    };

    if should_set {
        tbl.insert(key.to_string(), TomlValue::String(value.to_string()));
    }
}

fn format_version_for_about(ver: &Version) -> String {
    if ver.major > 0 {
        ver.major.to_string()
    } else {
        format!("0.{}", ver.minor)
    }
}
