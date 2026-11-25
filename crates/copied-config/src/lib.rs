use copied_core::{CopyTarget, CopyTargetDefinition, TARGETS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use thiserror::Error;

/// Behavior when the global keyboard shortcut fires.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShortcutCopyMode {
    #[default]
    CopyAndPick,
    Picker,
    CopyOnly,
}

/// Platform-neutral keyboard shortcut definition using W3C `KeyboardEvent.code`.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutSpec {
    pub code: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
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
            schema_version: 2,
            shortcut: default_shortcut(),
            shortcut_enabled: true,
            shortcut_mode: ShortcutCopyMode::CopyAndPick,
            last_target: None,
            format_visibility: BTreeMap::new(),
            picker_columns: 4,
            pinned_formats: BTreeMap::new(),
            has_launched: false,
        }
    }
}

/// Returns the platform default shortcut (`Ctrl+Cmd+C` on macOS, `Ctrl+Shift+Alt+C` elsewhere).
pub fn default_shortcut() -> ShortcutSpec {
    #[cfg(target_os = "macos")]
    {
        ShortcutSpec {
            code: String::from("KeyC"),
            ctrl: true,
            alt: false,
            shift: false,
            meta: true,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        ShortcutSpec {
            code: String::from("KeyC"),
            ctrl: true,
            alt: true,
            shift: true,
            meta: false,
        }
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
}

/// Resolved destination target and whether it came from an explicit per-app pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedTarget {
    pub target: CopyTarget,
    pub from_pin: bool,
}

/// Returns the non-empty list of visible formats (falling back to `[RichText]` if all are hidden).
pub fn visible_formats(visibility: &BTreeMap<CopyTarget, bool>) -> Vec<&'static CopyTargetDefinition> {
    let visible: Vec<_> = TARGETS
        .iter()
        .filter(|def| {
            visibility
                .get(&def.id)
                .copied()
                .unwrap_or(def.enabled_by_default != Some(false))
        })
        .collect();
    if visible.is_empty() {
        vec![CopyTarget::RichText.definition()]
    } else {
        visible
    }
}

/// Resolves the preferred format among visible formats.
pub fn preferred_format(
    visibility: &BTreeMap<CopyTarget, bool>,
    last_target: Option<CopyTarget>,
) -> CopyTarget {
    let visible = visible_formats(visibility);
    if let Some(target) = last_target
        && visible.iter().any(|def| def.id == target)
    {
        return target;
    }
    visible[0].id
}

/// Resolves the target for a shortcut invocation, honoring visible per-app pins without mutating `last_target`.
pub fn resolve_shortcut_target(config: &ConfigV2, app_id: Option<&str>) -> ResolvedTarget {
    let visible = visible_formats(&config.format_visibility);
    if let Some(id) = app_id
        && let Some(&pinned) = config.pinned_formats.get(id)
        && visible.iter().any(|def| def.id == pinned)
    {
        return ResolvedTarget {
            target: pinned,
            from_pin: true,
        };
    }
    ResolvedTarget {
        target: preferred_format(&config.format_visibility, config.last_target),
        from_pin: false,
    }
}

/// Loads `ConfigV2` from disk or returns defaults if missing.
pub fn load_config(path: &Path) -> Result<ConfigV2, ConfigError> {
    if !path.exists() {
        return Ok(ConfigV2::default());
    }
    let data = std::fs::read_to_string(path)?;
    let cfg: ConfigV2 = serde_json::from_str(&data)?;
    if cfg.schema_version != 2 {
        return Err(ConfigError::UnsupportedSchema(cfg.schema_version));
    }
    Ok(cfg)
}

/// Atomically writes `ConfigV2` to disk via a temp file + rename.
pub fn save_config(path: &Path, config: &ConfigV2) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut tmp, config)?;
    tmp.persist(path).map_err(|e| ConfigError::Io(e.error))?;
    Ok(())
}
