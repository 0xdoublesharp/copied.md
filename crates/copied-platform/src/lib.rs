use copied_config::ShortcutSpec;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Foreground application identity used for per-app pinned formats.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveApp {
    /// Namespaced ID (e.g. `mac:bundle:com.tinyspeck.slackmacgap`, `win:exe:slack.exe`, `x11:wmclass:Slack`).
    pub id: String,
    pub name: String,
}

/// How global shortcuts are managed on the active desktop session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShortcutCapability {
    Direct,
    Portal,
    Unsupported,
}

/// Runtime-probed OS capabilities (critical for Wayland / sandbox graceful degradation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilities {
    pub background_clipboard: bool,
    pub global_shortcuts: ShortcutCapability,
    pub active_app_detection: bool,
    pub launch_at_login: bool,
}

/// Launch-at-login registration state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LoginItemStatus {
    Disabled,
    Enabled,
    RequiresApproval,
    Unsupported,
}

/// Errors returned by platform backends.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PlatformError {
    #[error("shortcut already in use: {0}")]
    ShortcutInUse(String),
    #[error("shortcut registration failed: {0}")]
    ShortcutFailed(String),
    #[error("launch at login unavailable: {0}")]
    LoginItemFailed(String),
    #[error("unsupported on this platform: {0}")]
    Unsupported(String),
}

/// Global keyboard shortcut registration backend (register-first semantics).
pub trait ShortcutBackend: Send {
    fn current(&self) -> Option<&ShortcutSpec>;
    fn bind(&mut self, shortcut: &ShortcutSpec) -> Result<(), PlatformError>;
    fn unbind(&mut self);
}

/// Foreground application lookup for per-app pinned formats.
pub trait ActiveAppProvider: Send {
    fn frontmost(&self) -> Option<ActiveApp>;
}

/// Launch-at-login registration backend.
pub trait LoginItemBackend: Send {
    fn status(&self) -> LoginItemStatus;
    fn set_enabled(&mut self, enabled: bool) -> Result<LoginItemStatus, PlatformError>;
}
