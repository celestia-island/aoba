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
        title: String = "title".to_string(),
        com_ports: String = "com_ports".to_string(),
        details: String = "details".to_string(),
        no_com_ports: String = "no_com_ports".to_string(),
        help_short: String = "help_short".to_string(),
        auto_on: String = "auto_on".to_string(),
        auto_off: String = "auto_off".to_string(),
        last: String = "last".to_string(),
        last_none: String = "last_none".to_string(),
        name_label: String = "name_label".to_string(),
        type_label: String = "type_label".to_string(),
    details_placeholder: String = "details_placeholder".to_string(),
    press_c_clear: String = "press_c_clear".to_string(),
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
            // Fallback: return the default Lang (keys as values)
            Lang::default()
        }
    }
}

/// Return a reference to the currently selected `Lang`.
/// Callers can access fields directly, e.g. `i18n::lang().title`.
pub fn lang() -> &'static Lang {
    // If LANGUAGE hasn't been initialized, use the default Lang.
    LANG_SELECTED.get_or_init(|| Lang::default())
}

pub fn init_i18n() {
    // Available locales in priority order
    let mut avail: Vec<(&str, Lang)> = Vec::new();
    avail.push(("en_us", parse_toml_to_lang(EN_US_TOML)));
    avail.push(("zh_chs", parse_toml_to_lang(ZH_CHS_TOML)));
    avail.push(("zh_cht", parse_toml_to_lang(ZH_CHT_TOML)));

    // Detect preferred languages from env vars
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

    // Simple matcher: try to map prefs to available locales
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

    // Fallback to first available
    if chosen.is_none() {
        if let Some((k, l)) = avail.first() {
            chosen = Some((k, l.clone()));
        }
    }

    if let Some((k, l)) = chosen {
        LOCALE.set(k.to_string()).ok();
        LANG_SELECTED.set(l).ok();
    }

    let user = whoami::username();
    log::info!(
        "i18n: user={} locale={}",
        user,
        LOCALE.get().map(|s| s.as_str()).unwrap_or("-")
    );
}
