mod migrate;
mod schema;
mod shortcut;
mod store;
mod visibility;

pub use migrate::{
    LegacyMacDefaults, LegacyMacShortcut, merge_legacy_into_config, migrate_legacy_macos,
};
pub use schema::{ConfigError, ConfigPatch, ConfigV2, ShortcutCopyMode};
pub use shortcut::{
    ShortcutSpec, carbon_key_code_to_w3c, default_shortcut, w3c_to_carbon_key_code,
};
pub use store::{default_config_path, load_config, save_config};
pub use visibility::{
    ResolvedTarget, namespace_app_id, preferred_format, resolve_shortcut_target,
    set_format_visible, set_pinned_format, visible_formats,
};

#[cfg(test)]
mod tests;
