use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use yuuka::derive_struct;

const EN_US_TOML: &str = include_str!("../res/i18n/en_us.toml");
const ZH_CHS_TOML: &str = include_str!("../res/i18n/zh_chs.toml");
const ZH_CHT_TOML: &str = include_str!("../res/i18n/zh_cht.toml");

derive_struct! {
    #[derive(PartialEq, Serialize, Deserialize)]
    pub Lang {
        index: {
            title: String = "title".to_string(),
            home: String = "home".to_string(),
            com_ports: String = "com_ports".to_string(),
            details: String = "details".to_string(),
            no_com_ports: String = "no_com_ports".to_string(),
            auto_on: String = "auto_on".to_string(),
            auto_off: String = "auto_off".to_string(),
            last: String = "last".to_string(),
            last_none: String = "last_none".to_string(),
            name_label: String = "name_label".to_string(),
            type_label: String = "type_label".to_string(),
            details_placeholder: String = "details_placeholder".to_string(),

            port_header: String = "port_header".to_string(),
            state_header: String = "state_header".to_string(),
            port_state_free: String = "port_state_free".to_string(),
            port_state_owned: String = "port_state_owned".to_string(),
            port_state_other: String = "port_state_other".to_string(),
            refresh_action: String = "refresh_action".to_string(),
            manual_specify_label: String = "manual_specify_label".to_string(),
            about_label: String = "about_label".to_string(),
            manual_specify_linux_note: String = "manual_specify_linux_note".to_string(),
            manual_specify_unsupported: String = "manual_specify_unsupported".to_string(),
            scan_last_header: String = "scan_last_header".to_string(),
            scan_none: String = "scan_none".to_string(),
            scan_raw_header: String = "scan_raw_header".to_string(),
            scan_truncated_suffix: String = "scan_truncated_suffix".to_string(),
        },

        about: {
            welcome: String = "welcome".to_string(),
            name: String = "name".to_string(),
            version: String = "version".to_string(),
            authors: String = "authors".to_string(),
            repository: String = "repository".to_string(),
            license: String = "license".to_string(),
            dependencies: String = "dependencies".to_string(),
            dependencies_license_list: String = "dependencies_license_list".to_string(),
        },

        hotkeys: {
            // Template used to format key / label hints. Use {key} and {label} as placeholders.
            hint_kv_template: String = "hint_kv_template".to_string(),

            press_c_clear: String = "press_c_clear".to_string(),
            press_c_confirm: String = "press_c_confirm".to_string(),
            press_enter_toggle: String = "press_enter_toggle".to_string(),
            press_enter_select: String = "press_enter_select".to_string(),
            press_enter_enable: String = "press_enter_enable".to_string(),
            press_enter_release: String = "press_enter_release".to_string(),
            press_enter_unavailable: String = "press_enter_unavailable".to_string(),
            press_enter_refresh: String = "press_enter_refresh".to_string(),
            press_enter_manual_specify: String = "press_enter_manual_specify".to_string(),
            press_enter_modify: String = "press_enter_modify".to_string(),
            press_enter_submit: String = "press_enter_submit".to_string(),
            press_esc_cancel: String = "press_esc_cancel".to_string(),
            press_q_quit: String = "press_q_quit".to_string(),

            hint_enter_subpage: String = "hint_enter_subpage".to_string(),
            hint_back_list: String = "hint_back_list".to_string(),
            hint_switch_tab: String = "hint_switch_tab".to_string(),
            hint_move_vertical: String = "hint_move_vertical".to_string(),
            hint_move_with_panels: String = "hint_move_with_panels".to_string(),

            hint_master_enter_edit: String = "hint_master_enter_edit".to_string(),
            hint_master_editing: String = "hint_master_editing".to_string(),
            hint_master_edit_hex: String = "hint_master_edit_hex".to_string(),
            hint_master_edit_move: String = "hint_master_edit_move".to_string(),
            hint_master_edit_backspace: String = "hint_master_edit_backspace".to_string(),
            hint_master_edit_exit: String = "hint_master_edit_exit".to_string(),
            hint_esc_return_home: String = "hint_esc_return_home".to_string(),
            hint_master_delete: String = "hint_master_delete".to_string(),
            hint_master_field_select: String = "hint_master_field_select".to_string(),
            hint_master_field_move: String = "hint_master_field_move".to_string(),
            hint_master_field_exit_select: String = "hint_master_field_exit_select".to_string(),
            hint_master_field_apply: String = "hint_master_field_apply".to_string(),
            hint_master_field_cancel_edit: String = "hint_master_field_cancel_edit".to_string(),
            hint_master_type_switch: String = "hint_master_type_switch".to_string(),
            hint_reset_req_counter: String = "hint_reset_req_counter".to_string(),
        },

        tabs: {
            tab_config: String = "tab_config".to_string(),
            tab_log: String = "tab_log".to_string(),
            log_dir_send: String = "log_dir_send".to_string(),
            log_dir_recv: String = "log_dir_recv".to_string(),

            port_list: {
            },
            log: {
                hint_follow_on: String = "hint_follow_on".to_string(),
                hint_follow_off: String = "hint_follow_off".to_string(),
            }
        },

        input: {
            input_label: String = "input_label".to_string(),
            hint_input_edit: String = "hint_input_edit".to_string(),
            hint_input_mode: String = "hint_input_mode".to_string(),
            input_editing_hint: String = "input_editing_hint".to_string(),
            input_mode_ascii: String = "input_mode_ascii".to_string(),
            input_mode_hex: String = "input_mode_hex".to_string(),
            input_mode_current: String = "input_mode_current".to_string(),
            hint_input_edit_short: String = "hint_input_edit_short".to_string(),
            hint_input_mode_short: String = "hint_input_mode_short".to_string(),
        },

        protocol: {
            common: {
                serial_unknown: String = "serial_unknown".to_string(),
                label_port: String = "label_port".to_string(),
                label_type: String = "label_type".to_string(),
                label_status: String = "label_status".to_string(),
                label_mode: String = "label_mode".to_string(),
                label_baud: String = "label_baud".to_string(),
                label_data_bits: String = "label_data_bits".to_string(),
                label_parity: String = "label_parity".to_string(),
                label_stop_bits: String = "label_stop_bits".to_string(),
                label_guid: String = "label_guid".to_string(),
                label_mapping_code: String = "label_mapping_code".to_string(),
                label_usb: String = "label_usb".to_string(),
                mapping_none: String = "mapping_none".to_string(),
                label_serial: String = "label_serial".to_string(),
                label_manufacturer: String = "label_manufacturer".to_string(),
                label_product: String = "label_product".to_string(),
                custom: String = "custom".to_string(),
                parity_none: String = "parity_none".to_string(),
                parity_even: String = "parity_even".to_string(),
                parity_odd: String = "parity_odd".to_string(),
                toggle_too_fast: String = "toggle_too_fast".to_string(),
                enable_port: String = "enable_port".to_string(),
                protocol_selection: String = "protocol_selection".to_string(),
                configure_protocol: String = "configure_protocol".to_string(),
                port_enabled: String = "port_enabled".to_string(),
                port_disabled: String = "port_disabled".to_string(),
                mode_modbus: String = "mode_modbus".to_string(),
                mode_mqtt: String = "mode_mqtt".to_string(),
                mode_tcp: String = "mode_tcp".to_string(),
                mode_overlay_title: String = "mode_overlay_title".to_string(),
                mode_overlay_hint: String = "mode_overlay_hint".to_string(),
            },
            modbus: {
                edit_suffix: String = "edit_suffix".to_string(),
                registers_list: String = "registers_list".to_string(),
                label_master_list: String = "label_master_list".to_string(),
                label_slave_listen: String = "label_slave_listen".to_string(),
                label_modbus_settings: String = "label_modbus_settings".to_string(),
                label_working: String = "label_working".to_string(),
                status_running: String = "status_running".to_string(),
                status_paused: String = "status_paused".to_string(),
                label_master_passive: String = "label_master_passive".to_string(),
                status_master_passive: String = "status_master_passive".to_string(),
                status_master_active: String = "status_master_active".to_string(),
                label_default_marker: String = "label_default_marker".to_string(),
                hint_enter_toggle_work: String = "hint_enter_toggle_work".to_string(),
                hint_enter_start_work: String = "hint_enter_start_work".to_string(),
                hint_enter_pause_work: String = "hint_enter_pause_work".to_string(),
                invalid_baud_range: String = "invalid_baud_range".to_string(),
                new_master: String = "new_master".to_string(),
                new_slave: String = "new_slave".to_string(),
                new_entry: String = "new_entry".to_string(),
                label_address_range: String = "label_address_range".to_string(),
                reg_type_coils: String = "reg_type_coils".to_string(),
                reg_type_discrete_inputs: String = "reg_type_discrete_inputs".to_string(),
                reg_type_holding: String = "reg_type_holding".to_string(),
                reg_type_input: String = "reg_type_input".to_string(),
                label_req_counter: String = "label_req_counter".to_string(),
                refresh_rate: String = "refresh_rate".to_string(),
                global_interval: String = "global_interval".to_string(),
                global_timeout: String = "global_timeout".to_string(),
                value_true: String = "value_true".to_string(),
                value_false: String = "value_false".to_string(),
                role_master: String = "role_master".to_string(),
                role_slave: String = "role_slave".to_string(),
                log_sent_frame: String = "log_sent_frame".to_string(),
                log_unparsed_master: String = "log_unparsed_master".to_string(),
                log_recv_match: String = "log_recv_match".to_string(),
                log_recv_unmatched: String = "log_recv_unmatched".to_string(),
                log_reconfigured: String = "log_reconfigured".to_string(),
                log_req_timeout: String = "log_req_timeout".to_string(),
            },
            mqtt: {
                // Placeholder texts for MQTT panel (new)
                panel_placeholder: String = "panel_placeholder".to_string(),
                panel_not_current: String = "panel_not_current".to_string(),
            }
        },
    }
}

static LANG_SELECTED: OnceCell<Lang> = OnceCell::new();
static LOCALE: OnceCell<String> = OnceCell::new();

fn parse_toml_to_lang(content: &str) -> Lang {
    match toml::from_str::<Lang>(content) {
        Ok(data) => data,
        Err(err) => {
            log::warn!(
                "i18n: failed to parse toml: {}\ncontent preview: {}",
                err,
                &content.chars().take(200).collect::<String>()
            );
            // Fallback: return the default Lang (keys as values)
            Lang::default()
        }
    }
}

/// Return a reference to the currently selected `Lang`.
/// Callers can access fields directly, e.g. `i18n::lang().index.title`.
pub fn lang() -> &'static Lang {
    // If LANGUAGE hasn't been initialized, use the default Lang.
    LANG_SELECTED.get_or_init(Lang::default)
}

pub fn init_i18n() {
    // Available locales in priority order
    let avail: Vec<(&str, Lang)> = vec![
        ("en_us", parse_toml_to_lang(EN_US_TOML)),
        ("zh_chs", parse_toml_to_lang(ZH_CHS_TOML)),
        ("zh_cht", parse_toml_to_lang(ZH_CHT_TOML)),
    ];

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
            } else if let Some((_k, l)) = avail.iter().find(|(k, _)| *k == "zh_chs") {
                chosen = Some(("zh_chs", l.clone()));
                break;
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
