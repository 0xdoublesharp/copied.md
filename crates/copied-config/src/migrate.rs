use crate::schema::{DEFAULT_PICKER_COLUMNS, normalize_picker_columns};
use crate::{ConfigV2, ShortcutCopyMode, ShortcutSpec, default_shortcut, namespace_app_id};
use copied_core::resolve_target;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Shortcut decoded from the JSON data stored in `keyboardShortcut`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyMacShortcut {
    pub key_code: u32,
    pub modifiers: u32,
    pub key_label: String,
}

/// Values read from the legacy `md.copied.desktop` and `md.copied.clipboard` defaults.
/// Absent scalars stay unset, including an unreadable or absent shortcut blob.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LegacyMacDefaults {
    pub keyboard_shortcut: Option<LegacyMacShortcut>,
    pub keyboard_shortcut_disabled: Option<bool>,
    pub keyboard_shortcut_copy_mode: Option<i64>,
    pub has_launched_desktop_app: Option<bool>,
    #[serde(rename = "copied-md.pinnedFormats")]
    pub pinned_formats: BTreeMap<String, String>,
    #[serde(rename = "copied-md.lastTarget")]
    pub last_target: Option<String>,
    #[serde(rename = "copied-md.pickerHiddenFormats")]
    pub picker_hidden_formats: Vec<String>,
    #[serde(rename = "copied-md.pickerShownFormats")]
    pub picker_shown_formats: Vec<String>,
    #[serde(rename = "copied-md.pickerColumns")]
    pub picker_columns: Option<i64>,
}

/// Converts a snapshot of macOS preferences into v2 config.
pub fn migrate_legacy_macos(legacy: &LegacyMacDefaults) -> ConfigV2 {
    merge_legacy_into_config(&ConfigV2::default(), legacy)
}

/// Imports legacy settings into unused map entries and scalar fields at their defaults.
/// Existing non-default settings take precedence.
pub fn merge_legacy_into_config(existing: &ConfigV2, legacy: &LegacyMacDefaults) -> ConfigV2 {
    let defaults = ConfigV2::default();
    let mut config = existing.clone();

    if config.shortcut == defaults.shortcut || config.shortcut.validate().is_err() {
        config.shortcut = legacy
            .keyboard_shortcut
            .as_ref()
            .and_then(|shortcut| ShortcutSpec::from_carbon(shortcut.key_code, shortcut.modifiers))
            .filter(|shortcut| shortcut.validate().is_ok())
            .unwrap_or_else(default_shortcut);
    }
    if config.shortcut_enabled == defaults.shortcut_enabled {
        config.shortcut_enabled = !legacy.keyboard_shortcut_disabled.unwrap_or(false);
    }
    if config.shortcut_mode == defaults.shortcut_mode {
        config.shortcut_mode = match legacy.keyboard_shortcut_copy_mode {
            Some(1) => ShortcutCopyMode::Picker,
            Some(2) => ShortcutCopyMode::CopyOnly,
            _ => ShortcutCopyMode::CopyAndPick,
        };
    }
    if normalize_picker_columns(config.picker_columns) == DEFAULT_PICKER_COLUMNS {
        config.picker_columns = legacy
            .picker_columns
            .and_then(|columns| u8::try_from(columns).ok())
            .map_or(DEFAULT_PICKER_COLUMNS, normalize_picker_columns);
    }
    config.has_launched |= legacy.has_launched_desktop_app.unwrap_or(false);
    config.last_target = config
        .last_target
        .or_else(|| legacy.last_target.as_deref().and_then(resolve_target));

    // An explicit hide wins if both legacy lists reference the same target.
    for (ids, visible) in [
        (&legacy.picker_hidden_formats, false),
        (&legacy.picker_shown_formats, true),
    ] {
        for target in ids.iter().filter_map(|id| resolve_target(id)) {
            config.format_visibility.entry(target).or_insert(visible);
        }
    }
    let mut legacy_pins = BTreeMap::new();
    for (raw_id, target) in &legacy.pinned_formats {
        if let Some(target) = resolve_target(target) {
            let id = namespace_app_id("mac", raw_id);
            if &id == raw_id {
                legacy_pins.insert(id, target);
            } else {
                legacy_pins.entry(id).or_insert(target);
            }
        }
    }
    for (id, target) in legacy_pins {
        config.pinned_formats.entry(id).or_insert(target);
    }

    config.normalize();
    config
}
