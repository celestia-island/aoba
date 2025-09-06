use serde_json::Value as JsonValue;
use std::fs;
use std::process::Command;
use toml::value::{Table, Value as TomlValue};

fn main() {
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
                    for (k, val) in table.iter().take(500) {
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
                        let mut dep_t = Table::new();
                        dep_t.insert("name".to_string(), TomlValue::String(k.to_string()));
                        dep_t.insert("version".to_string(), TomlValue::String(ver));
                        darr.push(TomlValue::Table(dep_t));
                    }
                    out_tbl.insert("dependencies".to_string(), TomlValue::Array(darr));
                }
            }
        }
    }

    // Try cargo metadata for license map (metadata is JSON)
    match Command::new("cargo")
        .args(&["metadata", "--format-version", "1"])
        .output()
    {
        Ok(o) => {
            if o.status.success() {
                if let Ok(jv) = serde_json::from_slice::<JsonValue>(&o.stdout) {
                    if let Some(pkgs) = jv.get("packages").and_then(|p| p.as_array()) {
                        let mut map_tbl = Table::new();
                        for p in pkgs.iter() {
                            if let Some(n) = p.get("name").and_then(|x| x.as_str()) {
                                let lic = p.get("license").and_then(|x| x.as_str()).unwrap_or("");
                                let ver = p.get("version").and_then(|x| x.as_str()).unwrap_or("");
                                map_tbl.insert(
                                    format!("{}:{}", n, ver),
                                    TomlValue::String(lic.to_string()),
                                );
                                map_tbl.insert(n.to_string(), TomlValue::String(lic.to_string()));
                            }
                        }
                        out_tbl.insert("license_map".to_string(), TomlValue::Table(map_tbl));
                    }
                }
            }
        }
        Err(_) => {}
    }

    // write to res/about_cache.toml
    let toml_val = TomlValue::Table(out_tbl);
    if let Ok(s) = toml::to_string_pretty(&toml_val) {
        let _ = fs::create_dir_all("res");
        let _ = fs::write("res/about_cache.toml", s);
    }
}
