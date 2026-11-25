use crate::visibility::has_visible_formats;
use crate::{ShortcutSpec, default_shortcut};
use copied_core::CopyTarget;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub(crate) const SCHEMA_VERSION: u8 = 2;
pub(crate) const DEFAULT_PICKER_COLUMNS: u8 = 4;

/// Behavior when the global keyboard shortcut fires.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShortcutCopyMode {
    #[default]
    CopyAndPick,
    Picker,
    CopyOnly,
}

/// Versioned desktop configuration (`schemaVersion: 2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigV2 {
    pub schema_version: u8,
    pub shortcut: ShortcutSpec,
    pub shortcut_enabled: bool,
    pub shortcut_mode: ShortcutCopyMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_target: Option<CopyTarget>,
    #[serde(default)]
    pub format_visibility: BTreeMap<CopyTarget, bool>,
    pub picker_columns: u8,
    #[serde(default)]
    pub pinned_formats: BTreeMap<String, CopyTarget>,
    #[serde(default)]
    pub has_launched: bool,
}

impl Default for ConfigV2 {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            shortcut: default_shortcut(),
            shortcut_enabled: true,
            shortcut_mode: ShortcutCopyMode::CopyAndPick,
            last_target: None,
            format_visibility: BTreeMap::new(),
            picker_columns: DEFAULT_PICKER_COLUMNS,
            pinned_formats: BTreeMap::new(),
            has_launched: false,
        }
    }
}

/// Partial changes to a v2 config. Maps update their supplied entries individually.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<ShortcutSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortcut_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortcut_mode: Option<ShortcutCopyMode>,
    /// Omit to keep the previous target or send null to clear it.
    #[serde(
        deserialize_with = "deserialize_last_target",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_target: Option<Option<CopyTarget>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format_visibility: Option<BTreeMap<CopyTarget, bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picker_columns: Option<u8>,
    /// A null value removes the pin for that app ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned_formats: Option<BTreeMap<String, Option<CopyTarget>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_launched: Option<bool>,
}

#[expect(
    clippy::option_option,
    reason = "missing, null, and a target are distinct edits"
)]
fn deserialize_last_target<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Option<CopyTarget>>, D::Error> {
    Option::deserialize(deserializer).map(Some)
}

impl ConfigV2 {
    /// Applies all supplied changes together, leaving the config untouched on error.
    pub fn apply_patch(&mut self, patch: ConfigPatch) -> Result<(), ConfigError> {
        check_schema_version(self.schema_version)?;
        let mut updated = self.clone();
        if let Some(shortcut) = patch.shortcut {
            shortcut.validate()?;
            updated.shortcut = shortcut;
        }
        if let Some(visibility) = patch.format_visibility {
            updated.format_visibility.extend(visibility);
            if !has_visible_formats(&updated.format_visibility) {
                return Err(ConfigError::NoVisibleFormats);
            }
        }
        updated.shortcut_enabled = patch.shortcut_enabled.unwrap_or(updated.shortcut_enabled);
        updated.shortcut_mode = patch.shortcut_mode.unwrap_or(updated.shortcut_mode);
        updated.last_target = patch.last_target.unwrap_or(updated.last_target);
        updated.picker_columns = patch.picker_columns.unwrap_or(updated.picker_columns);
        updated.has_launched = patch.has_launched.unwrap_or(updated.has_launched);
        for (id, target) in patch.pinned_formats.into_iter().flatten() {
            crate::set_pinned_format(&mut updated, id, target);
        }
        updated.normalize();
        *self = updated;
        Ok(())
    }

    /// Repairs unusable settings in place, preserving remembered targets and pins.
    pub fn normalize(&mut self) {
        if self.shortcut.validate().is_err() {
            self.shortcut = default_shortcut();
        }
        self.picker_columns = normalize_picker_columns(self.picker_columns);
        if !has_visible_formats(&self.format_visibility) {
            self.format_visibility.insert(CopyTarget::RichText, true);
        }
    }
}

pub(crate) fn normalize_picker_columns(columns: u8) -> u8 {
    match columns {
        3..=5 => columns,
        _ => DEFAULT_PICKER_COLUMNS,
    }
}

pub(crate) fn check_schema_version(version: u8) -> Result<(), ConfigError> {
    if version == SCHEMA_VERSION {
        Ok(())
    } else {
        Err(ConfigError::UnsupportedSchema(version))
    }
}

/// Errors returned by configuration loading, validation, and atomic persistence.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported config schema version {0}")]
    UnsupportedSchema(u8),
    #[error("invalid shortcut: {0}")]
    InvalidShortcut(String),
    #[error("at least one copy format must remain visible")]
    NoVisibleFormats,
}
