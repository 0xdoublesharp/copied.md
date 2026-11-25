//! Fuzzes legacy macOS migration and the v2 config model.
//!
//! For every config reached through `migrate_legacy_macos`, `merge_legacy_into_config`,
//! `normalize`, `apply_patch` or JSON:
//!
//! * `visible_formats` is never empty;
//! * `preferred_format` is always one of the visible formats;
//! * `resolve_shortcut_target` never selects a hidden pin and always returns a visible
//!   format once the config is normalized;
//! * migration is deterministic and idempotent, and `normalize` is idempotent;
//! * normalized configs keep a valid shortcut and 3–5 picker columns and survive a JSON
//!   round trip.
#![no_main]

use arbitrary::Arbitrary;
use copied_config::{
    ConfigPatch, ConfigV2, LegacyMacDefaults, LegacyMacShortcut, ShortcutCopyMode, ShortcutSpec,
    merge_legacy_into_config, migrate_legacy_macos, preferred_format, resolve_shortcut_target,
    visible_formats,
};
use copied_core::{CopyTarget, TARGETS};
use libfuzzer_sys::fuzz_target;

const LEGACY_IDS: &[&str] = &[
    "html",
    "slack",
    "rich-text",
    "plain-text",
    "html-source",
    "slack-paste",
    "discord",
    "telegram",
    "signal",
    "reddit",
    "facebook",
    "twitter",
    "sms",
    "slack-api",
    "teams",
    "word",
    "whatsapp",
    "",
    "unknown",
    "RICH-TEXT",
];

#[derive(Arbitrary, Debug)]
enum TargetName {
    Known(u8),
    Raw(String),
}

impl TargetName {
    fn name(&self) -> String {
        match self {
            Self::Known(index) => LEGACY_IDS[usize::from(*index) % LEGACY_IDS.len()].to_owned(),
            Self::Raw(raw) => raw.clone(),
        }
    }
}

#[derive(Arbitrary, Debug)]
struct Legacy {
    shortcut: Option<(u8, u16, String)>,
    shortcut_disabled: Option<bool>,
    copy_mode: Option<i64>,
    has_launched: Option<bool>,
    pins: Vec<(String, TargetName)>,
    last_target: Option<TargetName>,
    hidden: Vec<TargetName>,
    shown: Vec<TargetName>,
    picker_columns: Option<i64>,
}

impl Legacy {
    fn defaults(&self) -> LegacyMacDefaults {
        LegacyMacDefaults {
            keyboard_shortcut: self.shortcut.as_ref().map(|(key, modifiers, label)| {
                LegacyMacShortcut {
                    // Carbon key codes live in 0..=0x7F; bias toward them.
                    key_code: u32::from(*key),
                    // Spread the bits over the Carbon modifier mask (cmd=256 … ctrl=4096).
                    modifiers: u32::from(*modifiers),
                    key_label: label.clone(),
                }
            }),
            keyboard_shortcut_disabled: self.shortcut_disabled,
            keyboard_shortcut_copy_mode: self.copy_mode,
            has_launched_desktop_app: self.has_launched,
            pinned_formats: self
                .pins
                .iter()
                .take(8)
                .map(|(id, target)| (id.clone(), target.name()))
                .collect(),
            last_target: self.last_target.as_ref().map(TargetName::name),
            picker_hidden_formats: self.hidden.iter().take(16).map(TargetName::name).collect(),
            picker_shown_formats: self.shown.iter().take(16).map(TargetName::name).collect(),
            picker_columns: self.picker_columns,
        }
    }
}

#[derive(Arbitrary, Debug)]
struct Config {
    key: u8,
    raw_code: Option<String>,
    modifiers: [bool; 4],
    shortcut_enabled: bool,
    mode: u8,
    last_target: Option<u8>,
    visibility: Vec<(u8, bool)>,
    picker_columns: u8,
    pins: Vec<(String, u8)>,
    has_launched: bool,
}

fn target(index: u8) -> CopyTarget {
    TARGETS[usize::from(index) % TARGETS.len()].id
}

impl Config {
    fn config(&self) -> ConfigV2 {
        const KEYS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let key = char::from(KEYS[usize::from(self.key) % KEYS.len()]);
        let code = self.raw_code.clone().unwrap_or_else(|| {
            if key.is_ascii_digit() {
                format!("Digit{key}")
            } else {
                format!("Key{key}")
            }
        });
        let [ctrl, alt, shift, meta] = self.modifiers;
        ConfigV2 {
            schema_version: 2,
            shortcut: ShortcutSpec {
                code,
                ctrl,
                alt,
                shift,
                meta,
            },
            shortcut_enabled: self.shortcut_enabled,
            shortcut_mode: match self.mode % 3 {
                0 => ShortcutCopyMode::CopyAndPick,
                1 => ShortcutCopyMode::Picker,
                _ => ShortcutCopyMode::CopyOnly,
            },
            last_target: self.last_target.map(target),
            format_visibility: self
                .visibility
                .iter()
                .take(20)
                .map(|&(index, visible)| (target(index), visible))
                .collect(),
            picker_columns: self.picker_columns,
            pinned_formats: self
                .pins
                .iter()
                .take(8)
                .map(|(id, index)| (id.clone(), target(*index)))
                .collect(),
            auto_detect_app_format: true,
            has_launched: self.has_launched,
        }
    }
}

#[derive(Arbitrary, Debug)]
enum Input {
    Migrate(Legacy),
    Merge(Config, Legacy),
    Config(Config, Vec<Option<String>>),
    Patch(Config, String),
    Json(String),
    LegacyJson(String),
}

fn is_visible(config: &ConfigV2, target: CopyTarget) -> bool {
    visible_formats(&config.format_visibility)
        .iter()
        .any(|def| def.id == target)
}

/// A pin is honored only when the map (or the target's default) makes it visible. Unlike
/// `visible_formats`, this has no RichText fallback when every format is hidden.
fn pin_visible(config: &ConfigV2, pin: CopyTarget) -> bool {
    config
        .format_visibility
        .get(&pin)
        .copied()
        .unwrap_or(pin.definition().enabled_by_default != Some(false))
}

/// Selection invariants that hold for any config, normalized or not.
fn check_selection(config: &ConfigV2, app_ids: &[Option<String>]) {
    let visible = visible_formats(&config.format_visibility);
    assert!(!visible.is_empty(), "no visible formats for {config:?}");
    let preferred = preferred_format(&config.format_visibility, config.last_target);
    assert!(
        is_visible(config, preferred),
        "preferred {preferred} hidden in {config:?}"
    );

    let pinned_ids = config.pinned_formats.keys().map(|id| Some(id.clone()));
    for app_id in app_ids
        .iter()
        .take(4)
        .cloned()
        .chain(pinned_ids)
        .chain([None])
    {
        let resolved = resolve_shortcut_target(config, app_id.as_deref());
        assert!(
            is_visible(config, resolved.target),
            "hidden target in {config:?}"
        );
        let pin = app_id.as_ref().and_then(|id| config.pinned_formats.get(id));
        let detected = app_id
            .as_deref()
            .filter(|_| config.auto_detect_app_format)
            .and_then(copied_config::detected_format_for_app);
        let expected_app_target = pin
            .copied()
            .filter(|&target| pin_visible(config, target))
            .or_else(|| detected.filter(|&target| pin_visible(config, target)));
        match expected_app_target {
            Some(target) => {
                assert!(resolved.from_pin, "expected app target {target} in {config:?}");
                assert_eq!(resolved.target, target);
            }
            None => {
                assert!(!resolved.from_pin, "unexpected app target in {config:?}");
                assert_eq!(resolved.target, preferred);
            }
        }
    }
}

/// Invariants for configs that went through `normalize`.
fn check_normalized(config: &ConfigV2, app_ids: &[Option<String>]) {
    check_selection(config, app_ids);
    config
        .shortcut
        .validate()
        .expect("normalized shortcut is valid");
    assert!((3..=5).contains(&config.picker_columns));
    // An explicit hide is never overridden: nothing hidden in the map is selectable.
    for (&target, &visible) in &config.format_visibility {
        assert_eq!(
            is_visible(config, target),
            visible,
            "{target} in {config:?}"
        );
    }
    let mut again = config.clone();
    again.normalize();
    assert_eq!(&again, config, "normalize is not idempotent");
    let json = serde_json::to_string(config).expect("serializable");
    let back: ConfigV2 = serde_json::from_str(&json).expect("round trip");
    assert_eq!(&back, config);
}

fn check_migration(legacy: &LegacyMacDefaults) {
    let migrated = migrate_legacy_macos(legacy);
    assert_eq!(
        migrate_legacy_macos(legacy),
        migrated,
        "non-deterministic migration"
    );
    assert_eq!(
        merge_legacy_into_config(&migrated, legacy),
        migrated,
        "not idempotent"
    );
    check_normalized(&migrated, &[]);
}

fuzz_target!(|input: Input| match input {
    Input::Migrate(legacy) => check_migration(&legacy.defaults()),
    Input::Merge(config, legacy) => {
        let existing = config.config();
        let legacy = legacy.defaults();
        let merged = merge_legacy_into_config(&existing, &legacy);
        check_normalized(&merged, &[]);
        assert_eq!(
            merge_legacy_into_config(&merged, &legacy),
            merged,
            "not idempotent"
        );
        // Existing pins and explicit visibility always win over legacy values.
        for (id, target) in &existing.pinned_formats {
            assert_eq!(merged.pinned_formats.get(id), Some(target));
        }
        for (target, visible) in &existing.format_visibility {
            if *visible {
                assert_eq!(merged.format_visibility.get(target), Some(&true));
            }
        }
        check_migration(&legacy);
    }
    Input::Config(config, app_ids) => {
        let mut config = config.config();
        check_selection(&config, &app_ids);
        config.normalize();
        check_normalized(&config, &app_ids);
    }
    Input::Patch(config, patch) => {
        let mut config = config.config();
        config.normalize();
        let before = config.clone();
        if let Ok(patch) = serde_json::from_str::<ConfigPatch>(&patch) {
            match config.apply_patch(patch) {
                Ok(()) => check_normalized(&config, &[]),
                Err(_) => assert_eq!(config, before, "failed patch mutated the config"),
            }
        }
    }
    Input::Json(json) => {
        if let Ok(mut config) = serde_json::from_str::<ConfigV2>(&json) {
            check_selection(&config, &[]);
            config.normalize();
            check_normalized(&config, &[]);
        }
    }
    Input::LegacyJson(json) => {
        if let Ok(legacy) = serde_json::from_str::<LegacyMacDefaults>(&json) {
            check_migration(&legacy);
        }
    }
});
