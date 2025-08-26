use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use yuuka::derive_struct;

// Include translation TOML at compile time
const EN_US_TOML: &str = include_str!("../res/i18n/en_us.toml");
const ZH_CHS_TOML: &str = include_str!("../res/i18n/zh_chs.toml");
const ZH_CHT_TOML: &str = include_str!("../res/i18n/zh_cht.toml");

derive_struct! {
    #[derive(PartialEq, Serialize, Deserialize)]
    pub Lang {
        title: String,
        com_ports: String,
        details: String,
        no_com_ports: String,
        help_short: String,
        auto_on: String,
        auto_off: String,
        last: String,
        last_none: String,
        name_label: String,
        type_label: String,
        details_placeholder: String,
    }
}

static LANG_SELECTED: OnceCell<Lang> = OnceCell::new();
static LOCALE: OnceCell<String> = OnceCell::new();

fn parse_toml_to_lang(content: &str) -> Lang {
    match toml::from_str::<Lang>(content) {
        Ok(l) => l,
        Err(e) => {
            log::warn!(
                "i18n: failed to parse toml: {}\ncontent preview: {}",
                e,
                &content.chars().take(200).collect::<String>()
            );
            // fallback: return a Lang with keys as values
            Lang {
                title: "title".to_string(),
                com_ports: "com_ports".to_string(),
                details: "details".to_string(),
                no_com_ports: "no_com_ports".to_string(),
                help_short: "help_short".to_string(),
                auto_on: "auto_on".to_string(),
                auto_off: "auto_off".to_string(),
                last: "last".to_string(),
                last_none: "last_none".to_string(),
                name_label: "name_label".to_string(),
                type_label: "type_label".to_string(),
                details_placeholder: "details_placeholder".to_string(),
            }
        }
    }
}

pub fn init_i18n() {
    // available locales in priority order
    let mut avail: Vec<(&str, Lang)> = Vec::new();
    avail.push(("en_us", parse_toml_to_lang(EN_US_TOML)));
    avail.push(("zh_chs", parse_toml_to_lang(ZH_CHS_TOML)));
    avail.push(("zh_cht", parse_toml_to_lang(ZH_CHT_TOML)));

    // detect preferred languages from env vars
    let mut prefs: Vec<String> = Vec::new();
    if let Ok(v) = std::env::var("LANGUAGE") {
        prefs.extend(v.split(':').map(|s| s.to_lowercase()));
    }
    if let Ok(v) = std::env::var("LC_ALL") {
        prefs.push(v.to_lowercase());
    }
    if let Ok(v) = std::env::var("LANG") {
        prefs.push(v.to_lowercase());
    }
    // Windows common env
    if let Ok(v) = std::env::var("USERLANGUAGE") {
        prefs.push(v.to_lowercase());
    }

    // simple matcher: try to map prefs to available locales
    let mut chosen: Option<(&str, Lang)> = None;
    for p in prefs.iter() {
        if p.contains("zh") {
            if p.contains("tw") || p.contains("hk") || p.contains("cht") {
                if let Some((_k, l)) = avail.iter().find(|(k, _)| *k == "zh_cht") {
                    chosen = Some(("zh_cht", l.clone()));
                    break;
                }
            } else {
                if let Some((_k, l)) = avail.iter().find(|(k, _)| *k == "zh_chs") {
                    chosen = Some(("zh_chs", l.clone()));
                    break;
                }
            }
        }
        if p.contains("en") {
            if let Some((_k, l)) = avail.iter().find(|(k, _)| *k == "en_us") {
                chosen = Some(("en_us", l.clone()));
                break;
            }
        }
    }

    // fallback to first available
    if chosen.is_none() {
        if let Some((k, l)) = avail.first() {
            chosen = Some((k, l.clone()));
        }
    }

    if let Some((k, l)) = chosen {
        LOCALE.set(k.to_string()).ok();
        LANG_SELECTED.set(l).ok();
    }

    // log whoami username and chosen locale
    let user = whoami::username();
    log::info!(
        "i18n: user={} locale={}",
        user,
        LOCALE.get().map(|s| s.as_str()).unwrap_or("-")
    );
}

pub fn tr(key: &str) -> String {
    if let Some(lang) = LANG_SELECTED.get() {
        match key {
            "title" => lang.title.clone(),
            "com_ports" => lang.com_ports.clone(),
            "details" => lang.details.clone(),
            "no_com_ports" => lang.no_com_ports.clone(),
            "help_short" => lang.help_short.clone(),
            "auto_on" => lang.auto_on.clone(),
            "auto_off" => lang.auto_off.clone(),
            "last" => lang.last.clone(),
            "last_none" => lang.last_none.clone(),
            "name_label" => lang.name_label.clone(),
            "type_label" => lang.type_label.clone(),
            "details_placeholder" => lang.details_placeholder.clone(),
            _ => key.to_string(),
        }
    } else {
        key.to_string()
    }
}
