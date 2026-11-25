use super::*;
use copied_core::{CopyTarget, TARGETS};
use directories::ProjectDirs;
use proptest::prelude::*;
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;

const SLACK: &str = "mac:bundle:com.tinyspeck.slackmacgap";

fn all_hidden() -> BTreeMap<CopyTarget, bool> {
    TARGETS.iter().map(|def| (def.id, false)).collect()
}

fn invalid_shortcut() -> ShortcutSpec {
    ShortcutSpec {
        code: "F4".into(),
        ..default_shortcut()
    }
}

#[test]
fn default_shortcut_is_valid_and_platform_specific() {
    let shortcut = default_shortcut();
    shortcut.validate().unwrap();
    let expected = if cfg!(target_os = "macos") {
        "Ctrl+Meta+C"
    } else {
        "Ctrl+Shift+Alt+C"
    };
    assert_eq!(shortcut.format_portable(), expected);
}

#[test]
fn parses_aliases_and_formats_modifiers_in_platform_order() {
    let shortcut = ShortcutSpec::parse(" Control + cMd + c ").unwrap();
    assert_eq!(shortcut.format_macos(), "⌃⌘C");
    assert_eq!(shortcut.format_portable(), "Ctrl+Meta+C");
    let shortcut = ShortcutSpec::parse("Cmd+Shift+Option+0").unwrap();
    assert_eq!(shortcut.code, "Digit0");
    assert_eq!(shortcut.format_macos(), "⌥⇧⌘0");
    assert_eq!(shortcut.format_portable(), "Shift+Alt+Meta+0");
    assert_eq!(
        ShortcutSpec::parse("alt+shift+ctrl+c")
            .unwrap()
            .format_portable(),
        "Ctrl+Shift+Alt+C"
    );
}

#[test]
fn rejects_invalid_shortcut_keys_and_modifier_combinations() {
    for label in [
        "",
        "C",
        "Ctrl+C",
        "Alt+Shift+C",
        "Ctrl+Control+C",
        "Cmd+Meta+C",
        "Ctrl+Hyper+C",
        "Ctrl++Alt+C",
        "Ctrl+Cmd+",
        "Ctrl+Cmd+C+D",
        "Ctrl+Cmd+F4",
        "Ctrl+Cmd+é",
        "Ctrl+Alt+13",
    ] {
        assert!(
            matches!(
                ShortcutSpec::parse(label),
                Err(ConfigError::InvalidShortcut(_))
            ),
            "{label:?}"
        );
    }

    for code in [
        "Keya",
        "KeyAA",
        "Digit10",
        "Key1",
        "DigitA",
        "F20",
        "ArrowLeft",
        "Numpad0",
    ] {
        let shortcut = ShortcutSpec {
            code: code.into(),
            ..default_shortcut()
        };
        assert!(
            matches!(shortcut.validate(), Err(ConfigError::InvalidShortcut(_))),
            "{code:?}"
        );
    }
}

#[test]
fn maps_all_ansi_alphanumeric_and_function_keys_in_both_directions() {
    let letters = [
        0, 11, 8, 2, 14, 3, 5, 4, 34, 38, 40, 37, 46, 45, 31, 35, 12, 15, 1, 17, 32, 9, 13, 7, 16,
        6,
    ];
    let digits = [29, 18, 19, 20, 21, 23, 22, 26, 28, 25];
    let function_keys = [
        122, 120, 99, 118, 96, 97, 98, 100, 101, 109, 103, 111, 105, 107, 113, 106, 64, 79, 80, 90,
    ];
    let mappings = (b'A'..=b'Z')
        .map(|letter| format!("Key{}", char::from(letter)))
        .zip(letters)
        .chain((0..=9).map(|digit| format!("Digit{digit}")).zip(digits))
        .chain(
            (1..=20)
                .map(|number| format!("F{number}"))
                .zip(function_keys),
        );

    for (code, carbon) in mappings {
        assert_eq!(carbon_key_code_to_w3c(carbon), Some(code.as_str()));
        assert_eq!(w3c_to_carbon_key_code(&code), Some(carbon));
    }
    assert_eq!(carbon_key_code_to_w3c(u32::MAX), None);
    assert_eq!(w3c_to_carbon_key_code("Space"), None);
    assert_eq!(w3c_to_carbon_key_code("F21"), None);
}

#[test]
fn decodes_supported_carbon_modifier_bits() {
    for (mask, flags) in [
        (4096, [true, false, false, false]),
        (2048, [false, true, false, false]),
        (512, [false, false, true, false]),
        (256, [false, false, false, true]),
        (1024, [false, false, false, false]),
        (4096 | 2048 | 512 | 256, [true, true, true, true]),
    ] {
        let shortcut = ShortcutSpec::from_carbon(0, mask).unwrap();
        assert_eq!(
            [shortcut.ctrl, shortcut.alt, shortcut.shift, shortcut.meta],
            flags
        );
    }
}

#[test]
fn resolves_visibility_defaults_and_preferred_format_in_catalog_order() {
    let mut visibility = BTreeMap::new();
    assert_eq!(visible_formats(&visibility).len(), 12);
    assert_eq!(
        preferred_format(&visibility, Some(CopyTarget::Teams)),
        CopyTarget::RichText
    );
    assert!(set_format_visible(&mut visibility, CopyTarget::Teams, true));
    assert!(set_format_visible(
        &mut visibility,
        CopyTarget::RichText,
        false
    ));
    assert_eq!(visible_formats(&visibility)[0].id, CopyTarget::PlainText);
    assert_eq!(
        preferred_format(&visibility, Some(CopyTarget::Teams)),
        CopyTarget::Teams
    );
    assert_eq!(
        preferred_format(&visibility, Some(CopyTarget::RichText)),
        CopyTarget::PlainText
    );
}

#[test]
fn rejects_hiding_the_last_visible_format_and_recovers_from_corrupt_visibility() {
    let mut visibility = all_hidden();
    assert_eq!(
        visible_formats(&visibility),
        [CopyTarget::RichText.definition()]
    );
    assert_eq!(
        preferred_format(&visibility, Some(CopyTarget::Word)),
        CopyTarget::RichText
    );
    assert!(!set_format_visible(
        &mut visibility,
        CopyTarget::RichText,
        false
    ));
    assert_eq!(visibility, all_hidden());
    assert!(set_format_visible(&mut visibility, CopyTarget::Teams, true));
    let before = visibility.clone();
    assert!(!set_format_visible(
        &mut visibility,
        CopyTarget::Teams,
        false
    ));
    assert_eq!(visibility, before);
}

#[test]
fn pins_override_recent_formats_while_visible_and_survive_hiding() {
    let mut config = ConfigV2 {
        last_target: Some(CopyTarget::PlainText),
        ..ConfigV2::default()
    };
    set_pinned_format(&mut config, SLACK.into(), Some(CopyTarget::SlackPaste));
    assert_eq!(
        resolve_shortcut_target(&config, Some(SLACK)),
        ResolvedTarget {
            target: CopyTarget::SlackPaste,
            from_pin: true,
        }
    );
    assert_eq!(config.last_target, Some(CopyTarget::PlainText));
    let fallback = ResolvedTarget {
        target: CopyTarget::PlainText,
        from_pin: false,
    };
    assert_eq!(resolve_shortcut_target(&config, None), fallback);
    assert_eq!(resolve_shortcut_target(&config, Some("other")), fallback);
    assert!(set_format_visible(
        &mut config.format_visibility,
        CopyTarget::SlackPaste,
        false
    ));
    assert_eq!(resolve_shortcut_target(&config, Some(SLACK)), fallback);
    assert_eq!(config.pinned_formats[SLACK], CopyTarget::SlackPaste);
    set_pinned_format(&mut config, SLACK.into(), None);
    assert!(config.pinned_formats.is_empty());

    config.format_visibility = all_hidden();
    set_pinned_format(&mut config, SLACK.into(), Some(CopyTarget::RichText));
    assert_eq!(
        resolve_shortcut_target(&config, Some(SLACK)),
        ResolvedTarget {
            target: CopyTarget::RichText,
            from_pin: false,
        }
    );
}

#[test]
fn namespacing_matches_the_desktop_application_id_formats() {
    assert_eq!(namespace_app_id("mac", "com.tinyspeck.slackmacgap"), SLACK);
    assert_eq!(namespace_app_id("macos", SLACK), SLACK);
    assert_eq!(
        namespace_app_id("windows", "slack.exe"),
        "win:exe:slack.exe"
    );
    assert_eq!(namespace_app_id("x11", "Slack"), "x11:wmclass:Slack");
    assert_eq!(
        namespace_app_id("wayland", "org.example"),
        "wayland:org.example"
    );
}

#[test]
fn patch_updates_only_supplied_fields_and_distinguishes_null_from_absent() {
    let mut config = ConfigV2 {
        last_target: Some(CopyTarget::HtmlSource),
        pinned_formats: BTreeMap::from([
            (SLACK.into(), CopyTarget::SlackPaste),
            ("x11:wmclass:other".into(), CopyTarget::PlainText),
        ]),
        format_visibility: BTreeMap::from([(CopyTarget::Word, true)]),
        ..ConfigV2::default()
    };
    let input = json!({
        "shortcut": {"code": "Digit9", "ctrl": true, "alt": true, "shift": false, "meta": false},
        "shortcutEnabled": false, "shortcutMode": "copy-only", "pickerColumns": 99,
        "formatVisibility": {"teams": true}, "hasLaunched": true,
        "pinnedFormats": {SLACK: null, "mac:bundle:org.example": "teams"},
    });
    let patch: ConfigPatch = serde_json::from_value(input.clone()).unwrap();
    assert_eq!(serde_json::to_value(&patch).unwrap(), input);
    config.apply_patch(patch).unwrap();
    assert_eq!(config.last_target, Some(CopyTarget::HtmlSource));
    assert_eq!(config.shortcut.format_portable(), "Ctrl+Alt+9");
    assert!(!config.shortcut_enabled);
    assert_eq!(config.shortcut_mode, ShortcutCopyMode::CopyOnly);
    assert_eq!(config.picker_columns, 4);
    assert!(config.has_launched);
    assert!(config.format_visibility[&CopyTarget::Word]);
    assert!(config.format_visibility[&CopyTarget::Teams]);
    assert_eq!(config.pinned_formats.len(), 2);
    assert!(!config.pinned_formats.contains_key(SLACK));
    assert_eq!(
        config.pinned_formats["mac:bundle:org.example"],
        CopyTarget::Teams
    );

    for (value, target) in [
        (json!(null), None),
        (json!("discord"), Some(CopyTarget::Discord)),
    ] {
        let input = json!({"lastTarget": value});
        let patch: ConfigPatch = serde_json::from_value(input.clone()).unwrap();
        assert_eq!(serde_json::to_value(&patch).unwrap(), input);
        config.apply_patch(patch).unwrap();
        assert_eq!(config.last_target, target);
    }
    assert!(serde_json::from_value::<ConfigPatch>(json!({"schemaVersion": 9})).is_err());
}

#[test]
fn patch_is_transactional_when_shortcut_or_visibility_is_invalid() {
    let mut config = ConfigV2::default();
    let before = config.clone();
    let shortcut = ConfigPatch {
        shortcut: Some(invalid_shortcut()),
        shortcut_enabled: Some(false),
        ..ConfigPatch::default()
    };
    assert!(matches!(
        config.apply_patch(shortcut),
        Err(ConfigError::InvalidShortcut(_))
    ));
    assert_eq!(config, before);

    let mut visibility = all_hidden();
    let patch = ConfigPatch {
        format_visibility: Some(visibility.clone()),
        picker_columns: Some(5),
        ..ConfigPatch::default()
    };
    assert!(matches!(
        config.apply_patch(patch),
        Err(ConfigError::NoVisibleFormats)
    ));
    assert_eq!(config, before);

    visibility.insert(CopyTarget::Teams, true);
    assert!(
        config
            .apply_patch(ConfigPatch {
                format_visibility: Some(visibility),
                picker_columns: Some(5),
                ..ConfigPatch::default()
            })
            .is_ok()
    );
    assert_eq!(
        visible_formats(&config.format_visibility),
        [CopyTarget::Teams.definition()]
    );
    assert_eq!(config.picker_columns, 5);
}

#[test]
fn migrates_every_user_defaults_key_and_preserves_legacy_visibility_precedence() {
    let legacy: LegacyMacDefaults = serde_json::from_value(json!({
        "keyboardShortcut": {"keyCode": 8, "modifiers": 6656, "keyLabel": "J"},
        "keyboardShortcutDisabled": true,
        "keyboardShortcutCopyMode": 2,
        "hasLaunchedDesktopApp": true,
        "copied-md.pinnedFormats": {"com.tinyspeck.slackmacgap": "slack", "unknown": "removed"},
        "copied-md.lastTarget": "html",
        "copied-md.pickerHiddenFormats": ["slack", "word", "gone"],
        "copied-md.pickerShownFormats": ["word", "teams", "discord", "gone"],
        "copied-md.pickerColumns": 5,
    }))
    .unwrap();

    let migrated = migrate_legacy_macos(&legacy);
    assert_eq!(migrated.schema_version, 2);
    assert_eq!(migrated.shortcut.format_portable(), "Ctrl+Shift+Alt+C");
    assert!(!migrated.shortcut_enabled);
    assert_eq!(migrated.shortcut_mode, ShortcutCopyMode::CopyOnly);
    assert!(migrated.has_launched);
    assert_eq!(migrated.last_target, Some(CopyTarget::RichText));
    assert_eq!(migrated.picker_columns, 5);
    assert_eq!(
        migrated.format_visibility,
        BTreeMap::from([
            (CopyTarget::SlackPaste, false),
            (CopyTarget::Teams, true),
            (CopyTarget::Word, false),
            (CopyTarget::Discord, true),
        ])
    );
    assert_eq!(
        migrated.pinned_formats,
        BTreeMap::from([(SLACK.into(), CopyTarget::SlackPaste)])
    );
    assert_eq!(
        preferred_format(&migrated.format_visibility, migrated.last_target),
        CopyTarget::RichText
    );
    assert_eq!(merge_legacy_into_config(&migrated, &legacy), migrated);
}

#[test]
fn migration_handles_missing_invalid_and_corrupt_legacy_settings() {
    let no_defaults: LegacyMacDefaults = serde_json::from_value(json!({})).unwrap();
    assert_eq!(migrate_legacy_macos(&no_defaults), ConfigV2::default());
    assert!(serde_json::from_slice::<LegacyMacShortcut>(b"{malformed").is_err());

    for (key_code, modifiers) in [(u32::MAX, 4352), (122, 4352), (8, 4096)] {
        let legacy = LegacyMacDefaults {
            keyboard_shortcut: Some(LegacyMacShortcut {
                key_code,
                modifiers,
                key_label: "C".into(),
            }),
            keyboard_shortcut_copy_mode: Some(-1),
            keyboard_shortcut_disabled: Some(false),
            has_launched_desktop_app: Some(false),
            picker_columns: Some(i64::MAX),
            last_target: Some("unknown".into()),
            picker_hidden_formats: TARGETS.iter().map(|def| def.id.id().into()).collect(),
            ..LegacyMacDefaults::default()
        };
        let migrated = migrate_legacy_macos(&legacy);
        assert_eq!(migrated.shortcut, default_shortcut());
        assert!(migrated.shortcut_enabled);
        assert!(!migrated.has_launched);
        assert_eq!(migrated.shortcut_mode, ShortcutCopyMode::CopyAndPick);
        assert_eq!(migrated.picker_columns, 4);
        assert_eq!(migrated.last_target, None);
        assert!(migrated.format_visibility[&CopyTarget::RichText]);
        assert_eq!(
            visible_formats(&migrated.format_visibility),
            [CopyTarget::RichText.definition()]
        );
        assert_eq!(merge_legacy_into_config(&migrated, &legacy), migrated);
    }
}

#[test]
fn merge_preserves_existing_settings_and_normalizes_legacy_pin_ids_once() {
    let legacy = LegacyMacDefaults {
        keyboard_shortcut: Some(LegacyMacShortcut {
            key_code: 8,
            modifiers: 4352,
            key_label: "C".into(),
        }),
        keyboard_shortcut_copy_mode: Some(1),
        keyboard_shortcut_disabled: Some(true),
        has_launched_desktop_app: Some(false),
        pinned_formats: BTreeMap::from([
            (SLACK.into(), "slack".into()),
            ("com.tinyspeck.slackmacgap".into(), "discord".into()),
            ("mac:bundle:z.example".into(), "teams".into()),
            ("z.example".into(), "word".into()),
        ]),
        last_target: Some("slack".into()),
        picker_shown_formats: vec!["teams".into(), "word".into()],
        picker_columns: Some(5),
        ..LegacyMacDefaults::default()
    };
    let migrated = migrate_legacy_macos(&legacy);
    assert_eq!(migrated.shortcut_mode, ShortcutCopyMode::Picker);
    assert_eq!(migrated.pinned_formats.len(), 2);
    assert_eq!(migrated.pinned_formats[SLACK], CopyTarget::SlackPaste);
    assert_eq!(
        migrated.pinned_formats["mac:bundle:z.example"],
        CopyTarget::Teams
    );

    let existing = ConfigV2 {
        shortcut: ShortcutSpec::parse("Ctrl+Alt+Z").unwrap(),
        shortcut_enabled: false,
        shortcut_mode: ShortcutCopyMode::CopyOnly,
        last_target: Some(CopyTarget::Discord),
        picker_columns: 3,
        has_launched: true,
        pinned_formats: BTreeMap::from([(SLACK.into(), CopyTarget::Discord)]),
        format_visibility: BTreeMap::from([(CopyTarget::Teams, false)]),
        ..ConfigV2::default()
    };
    let merged = merge_legacy_into_config(&existing, &legacy);
    let mut expected = existing;
    expected
        .pinned_formats
        .insert("mac:bundle:z.example".into(), CopyTarget::Teams);
    expected.format_visibility.insert(CopyTarget::Word, true);
    assert_eq!(merged, expected);
    assert_eq!(merge_legacy_into_config(&merged, &legacy), merged);

    let corrupt = ConfigV2 {
        shortcut: invalid_shortcut(),
        picker_columns: 255,
        ..ConfigV2::default()
    };
    assert_eq!(merge_legacy_into_config(&corrupt, &legacy), migrated);
}

#[test]
fn finds_default_path_and_loads_defaults_for_missing_files() {
    let dirs = ProjectDirs::from("md", "copied", "copied.md").unwrap();
    assert_eq!(
        default_config_path(),
        Some(dirs.config_dir().join("config.json"))
    );
    let directory = tempfile::tempdir().unwrap();
    assert_eq!(
        load_config(&directory.path().join("config.json")).unwrap(),
        ConfigV2::default()
    );
    assert!(matches!(
        load_config(directory.path()),
        Err(ConfigError::Io(_))
    ));
}

#[test]
fn disk_load_repairs_invalid_values_and_preserves_the_remembered_target() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    let config = ConfigV2 {
        picker_columns: 9,
        shortcut: invalid_shortcut(),
        format_visibility: all_hidden(),
        last_target: Some(CopyTarget::Word),
        ..ConfigV2::default()
    };
    fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    let loaded = load_config(&path).unwrap();
    assert_eq!(loaded.picker_columns, 4);
    assert_eq!(loaded.shortcut, default_shortcut());
    assert_eq!(loaded.last_target, config.last_target);
    assert_eq!(
        visible_formats(&loaded.format_visibility),
        [CopyTarget::RichText.definition()]
    );
}

#[test]
fn disk_store_rejects_bad_json_and_incompatible_versions() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    for input in [
        r"{malformed",
        "{}",
        r#"{"schemaVersion":2,"shortcut":"invalid"}"#,
        r#"{"schemaVersion":2,"schemaVersion":2}"#,
    ] {
        fs::write(&path, input).unwrap();
        assert!(matches!(load_config(&path), Err(ConfigError::Json(_))));
    }
    let future = r#"{"schemaVersion":3,"shortcut":{"futureShape":true}}"#;
    fs::write(&path, future).unwrap();
    assert!(matches!(
        load_config(&path),
        Err(ConfigError::UnsupportedSchema(3))
    ));
    let config = ConfigV2 {
        schema_version: 1,
        ..ConfigV2::default()
    };
    assert!(matches!(
        save_config(&path, &config),
        Err(ConfigError::UnsupportedSchema(1))
    ));
    assert_eq!(fs::read_to_string(&path).unwrap(), future);
}

#[test]
fn atomic_save_replaces_config_and_cleans_up_when_rename_fails() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nested/config.json");
    let config = ConfigV2::default();
    save_config(&path, &config).unwrap();
    let updated = ConfigV2 {
        shortcut_enabled: false,
        picker_columns: 3,
        ..config
    };
    save_config(&path, &updated).unwrap();
    assert_eq!(load_config(&path).unwrap(), updated);
    let obstacle = path.with_file_name("obstacle");
    fs::create_dir(&obstacle).unwrap();
    assert!(matches!(
        save_config(&obstacle, &updated),
        Err(ConfigError::Io(_))
    ));
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 2);
    assert_eq!(load_config(&path).unwrap(), updated);
}

#[test]
fn concurrent_saves_publish_only_complete_configs() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    let first = ConfigV2 {
        picker_columns: 3,
        ..ConfigV2::default()
    };
    let second = ConfigV2 {
        picker_columns: 5,
        ..ConfigV2::default()
    };
    save_config(&path, &first).unwrap();
    let path = &path;
    std::thread::scope(|scope| {
        for config in [&first, &second] {
            scope.spawn(move || {
                for _ in 0..8 {
                    save_config(path, config).unwrap();
                }
            });
        }
        for _ in 0..32 {
            let config = load_config(path).unwrap();
            assert!(config == first || config == second);
        }
    });
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
}

fn target_strategy() -> impl Strategy<Value = CopyTarget> {
    (0..TARGETS.len()).prop_map(|index| TARGETS[index].id)
}

fn visibility_strategy() -> impl Strategy<Value = BTreeMap<CopyTarget, bool>> {
    prop::collection::btree_map(target_strategy(), any::<bool>(), 0..=TARGETS.len())
}

fn shortcut_strategy() -> impl Strategy<Value = ShortcutSpec> {
    (
        prop_oneof![
            (b'A'..=b'Z').prop_map(char::from),
            (b'0'..=b'9').prop_map(char::from)
        ],
        0b0011_u32..=0b1111,
    )
        .prop_filter(
            "at least two modifiers, including Ctrl or Meta",
            |(_, mask)| mask.count_ones() >= 2 && mask & 0b1001 != 0,
        )
        .prop_map(|(key, mask)| ShortcutSpec {
            code: if key.is_ascii_digit() {
                format!("Digit{key}")
            } else {
                format!("Key{key}")
            },
            ctrl: mask & 1 != 0,
            alt: mask & 2 != 0,
            shift: mask & 4 != 0,
            meta: mask & 8 != 0,
        })
}

proptest! {
    #![proptest_config(ProptestConfig { failure_persistence: None, ..ProptestConfig::default() })]

    #[test]
    fn visibility_preference_and_pins_agree(
        mut visibility in visibility_strategy(),
        last_target in proptest::option::of(target_strategy()),
        pin in target_strategy(),
        target in target_strategy(),
        enabled in any::<bool>(),
    ) {
        let previous = visibility.clone();
        if !set_format_visible(&mut visibility, target, enabled) {
            prop_assert_eq!(&visibility, &previous);
        }
        let visible = visible_formats(&visibility);
        prop_assert!(!visible.is_empty());
        if TARGETS.iter().any(|def| previous.get(&def.id)
            .copied().unwrap_or(def.enabled_by_default != Some(false))) {
            prop_assert!(visible.iter().any(|def| visibility.get(&def.id)
                .copied().unwrap_or(def.enabled_by_default != Some(false))));
        }

        let preferred = preferred_format(&visibility, last_target);
        prop_assert!(visible.iter().any(|def| def.id == preferred));
        let config = ConfigV2 {
            format_visibility: visibility.clone(),
            last_target,
            pinned_formats: BTreeMap::from([(SLACK.into(), pin)]),
            ..ConfigV2::default()
        };
        let resolved = resolve_shortcut_target(&config, Some(SLACK));
        let pin_visible = visibility.get(&pin).copied()
            .unwrap_or(pin.definition().enabled_by_default != Some(false));
        prop_assert_eq!(resolved.from_pin, pin_visible);
        prop_assert_eq!(resolved.target, if pin_visible { pin } else { preferred });
    }

    #[test]
    fn shortcuts_round_trip_through_portable_notation(shortcut in shortcut_strategy()) {
        prop_assert!(shortcut.validate().is_ok());
        prop_assert_eq!(ShortcutSpec::parse(&shortcut.format_portable()).unwrap(), shortcut);
    }

    #[test]
    fn migration_is_idempotent_and_keeps_recognized_keys(
        pin in target_strategy(),
        last in target_strategy(),
        shown in target_strategy(),
        hidden in target_strategy(),
        key_code in 0_u32..128,
        columns in -1_i64..=6,
        disabled in any::<bool>(),
        launched in any::<bool>(),
    ) {
        let legacy = LegacyMacDefaults {
            keyboard_shortcut: Some(LegacyMacShortcut { key_code, modifiers: 4352, key_label: "C".into() }),
            keyboard_shortcut_disabled: Some(disabled),
            keyboard_shortcut_copy_mode: Some(1),
            has_launched_desktop_app: Some(launched),
            pinned_formats: BTreeMap::from([(SLACK.into(), pin.id().into())]),
            last_target: Some(last.id().into()),
            picker_hidden_formats: vec![hidden.id().into()],
            picker_shown_formats: vec![shown.id().into()],
            picker_columns: Some(columns),
        };
        let migrated = migrate_legacy_macos(&legacy);
        prop_assert_eq!(&migrate_legacy_macos(&legacy), &migrated);
        prop_assert_eq!(&merge_legacy_into_config(&migrated, &legacy), &migrated);
        prop_assert_eq!(migrated.pinned_formats.len(), 1);
        prop_assert_eq!(migrated.pinned_formats[SLACK], pin);
        prop_assert_eq!(migrated.last_target, Some(last));
        prop_assert!(!migrated.format_visibility[&hidden]);
        prop_assert_eq!(migrated.format_visibility[&shown], shown != hidden);
        prop_assert_eq!(migrated.shortcut_enabled, !disabled);
        prop_assert_eq!(migrated.has_launched, launched);
        prop_assert_eq!(migrated.shortcut_mode, ShortcutCopyMode::Picker);
        prop_assert_eq!(i64::from(migrated.picker_columns), if (3..=5).contains(&columns) { columns } else { 4 });
    }

    #[test]
    fn normalized_configs_round_trip_through_disk(
        visibility in visibility_strategy(),
        last_target in proptest::option::of(target_strategy()),
        pin in target_strategy(),
        shortcut in shortcut_strategy(),
        picker_columns in any::<u8>(),
        shortcut_enabled in any::<bool>(),
        has_launched in any::<bool>(),
    ) {
        let config = ConfigV2 {
            format_visibility: visibility, last_target, shortcut, picker_columns,
            pinned_formats: BTreeMap::from([(SLACK.into(), pin)]),
            shortcut_enabled, has_launched, ..ConfigV2::default()
        };
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.json");
        save_config(&path, &config).unwrap();
        let mut expected = config;
        expected.normalize();
        prop_assert_eq!(load_config(&path).unwrap(), expected);
    }
}
