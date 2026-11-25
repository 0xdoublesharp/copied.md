use crate::ConfigV2;
use copied_core::{CopyTarget, CopyTargetDefinition, TARGETS};
use std::collections::BTreeMap;

/// Resolved destination target and whether it came from an explicit per-app pin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedTarget {
    pub target: CopyTarget,
    pub from_pin: bool,
}

fn is_visible(visibility: &BTreeMap<CopyTarget, bool>, def: &CopyTargetDefinition) -> bool {
    visibility
        .get(&def.id)
        .copied()
        .unwrap_or(def.enabled_by_default != Some(false))
}

pub(crate) fn has_visible_formats(visibility: &BTreeMap<CopyTarget, bool>) -> bool {
    TARGETS.iter().any(|def| is_visible(visibility, def))
}

/// Returns the non-empty list of visible formats (falling back to `[RichText]` if all are hidden).
pub fn visible_formats(
    visibility: &BTreeMap<CopyTarget, bool>,
) -> Vec<&'static CopyTargetDefinition> {
    let visible: Vec<_> = TARGETS
        .iter()
        .filter(|def| is_visible(visibility, def))
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
    if let Some(target) = last_target
        && is_visible(visibility, target.definition())
    {
        return target;
    }
    TARGETS
        .iter()
        .find(|def| is_visible(visibility, def))
        .map_or(CopyTarget::RichText, |def| def.id)
}

/// Maps well-known destination app IDs across macOS, Windows, and Linux to their native format.
pub fn detected_format_for_app(app_id: &str) -> Option<CopyTarget> {
    let raw = app_id.rsplit(':').next().unwrap_or(app_id);
    match raw.to_ascii_lowercase().as_str() {
        "com.tinyspeck.slackmacgap" | "slack.exe" | "slack" => Some(CopyTarget::SlackPaste),
        "com.hnc.discord"
        | "com.hnc.discordptb"
        | "com.hnc.discordcanary"
        | "discord.exe"
        | "discord" => Some(CopyTarget::Discord),
        "ru.keepcoder.telegram"
        | "org.telegram.desktop"
        | "telegram.exe"
        | "telegram-desktop"
        | "telegramdesktop" => Some(CopyTarget::Telegram),
        "org.whispersystems.signal-desktop" | "signal.exe" | "signal" => Some(CopyTarget::Signal),
        "com.microsoft.teams2"
        | "com.microsoft.teams"
        | "ms-teams.exe"
        | "teams.exe"
        | "teams-for-linux" => Some(CopyTarget::Teams),
        "com.microsoft.word" | "winword.exe" => Some(CopyTarget::Word),
        "net.whatsapp.whatsapp" | "whatsapp.exe" | "whatsapp" => Some(CopyTarget::Whatsapp),
        "com.apple.mobilesms" => Some(CopyTarget::Sms),
        _ => None,
    }
}

/// Resolves the target for a shortcut invocation, honoring visible per-app pins and optional
/// destination auto-detection without mutating `last_target`.
pub fn resolve_shortcut_target(config: &ConfigV2, app_id: Option<&str>) -> ResolvedTarget {
    if let Some(id) = app_id {
        if let Some(&pinned) = config.pinned_formats.get(id)
            && is_visible(&config.format_visibility, pinned.definition())
        {
            return ResolvedTarget {
                target: pinned,
                from_pin: true,
            };
        }
        if config.auto_detect_app_format
            && let Some(detected) = detected_format_for_app(id)
            && is_visible(&config.format_visibility, detected.definition())
        {
            return ResolvedTarget {
                target: detected,
                from_pin: true,
            };
        }
    }
    ResolvedTarget {
        target: preferred_format(&config.format_visibility, config.last_target),
        from_pin: false,
    }
}

/// Sets visibility, returning false if the change would hide every format.
pub fn set_format_visible(
    visibility: &mut BTreeMap<CopyTarget, bool>,
    target: CopyTarget,
    visible: bool,
) -> bool {
    if !visible
        && !TARGETS
            .iter()
            .any(|def| def.id != target && is_visible(visibility, def))
    {
        return false;
    }
    visibility.insert(target, visible);
    true
}

/// Sets or clears a per-app pin keyed by its namespaced application ID.
pub fn set_pinned_format(config: &mut ConfigV2, app_id: String, target: Option<CopyTarget>) {
    if let Some(target) = target {
        config.pinned_formats.insert(app_id, target);
    } else {
        config.pinned_formats.remove(&app_id);
    }
}

/// Adds a platform namespace, preserving IDs already in that namespace.
pub fn namespace_app_id(platform: &str, raw_id: &str) -> String {
    let namespace = match platform {
        "mac" | "macos" => "mac:bundle",
        "win" | "windows" => "win:exe",
        "x11" => "x11:wmclass",
        _ => platform,
    };
    let prefix = format!("{namespace}:");
    if raw_id.starts_with(&prefix) {
        raw_id.to_owned()
    } else {
        format!("{prefix}{raw_id}")
    }
}
