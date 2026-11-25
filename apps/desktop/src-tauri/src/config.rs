use std::path::{Path, PathBuf};

use copied_config::{ConfigV2, default_config_path, load_config, save_config};

use crate::DesktopError;

pub(crate) fn open_config() -> Result<(PathBuf, ConfigV2), DesktopError> {
    let path = default_config_path().ok_or(DesktopError::MissingConfigPath)?;
    let config = load_or_import(&path)?;
    Ok((path, config))
}

fn load_or_import(path: &Path) -> Result<ConfigV2, DesktopError> {
    // A first-run import must never replace an unreadable or incompatible config.
    if path.try_exists()? {
        return Ok(load_config(path)?);
    }
    #[cfg(target_os = "macos")]
    let config = legacy_macos::import();
    #[cfg(not(target_os = "macos"))]
    let config = ConfigV2::default();

    save_config(path, &config)?;
    Ok(config)
}

#[cfg(target_os = "macos")]
mod legacy_macos {
    use std::collections::BTreeMap;

    use copied_config::{ConfigV2, LegacyMacDefaults, migrate_legacy_macos};
    use objc2::{DowncastTarget, rc::Retained, runtime::AnyObject};
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSNumber, NSString, NSUserDefaults};

    type Domain = NSDictionary<NSString, AnyObject>;

    pub(super) fn import() -> ConfigV2 {
        let defaults = NSUserDefaults::standardUserDefaults();
        let desktop = defaults.persistentDomainForName(&NSString::from_str("md.copied.desktop"));
        let clipboard =
            defaults.persistentDomainForName(&NSString::from_str("md.copied.clipboard"));
        let desktop = desktop.as_deref();
        let clipboard = clipboard.as_deref();

        migrate_legacy_macos(&LegacyMacDefaults {
            keyboard_shortcut: item::<NSData>(desktop, "keyboardShortcut")
                .and_then(|data| serde_json::from_slice(&data.to_vec()).ok()),
            keyboard_shortcut_disabled: item::<NSNumber>(desktop, "keyboardShortcutDisabled")
                .map(|number| number.boolValue()),
            keyboard_shortcut_copy_mode: integer(desktop, "keyboardShortcutCopyMode"),
            has_launched_desktop_app: item::<NSNumber>(desktop, "hasLaunchedDesktopApp")
                .map(|number| number.boolValue()),
            pinned_formats: pins(desktop),
            last_target: item::<NSString>(clipboard, "copied-md.lastTarget")
                .map(|value| value.to_string()),
            picker_hidden_formats: strings(clipboard, "copied-md.pickerHiddenFormats"),
            picker_shown_formats: strings(clipboard, "copied-md.pickerShownFormats"),
            picker_columns: integer(clipboard, "copied-md.pickerColumns"),
        })
    }

    fn item<T: DowncastTarget>(domain: Option<&Domain>, key: &str) -> Option<Retained<T>> {
        domain?
            .objectForKey(&NSString::from_str(key))?
            .downcast()
            .ok()
    }

    fn integer(domain: Option<&Domain>, key: &str) -> Option<i64> {
        item::<NSNumber>(domain, key).map(|number| number.longLongValue())
    }

    fn strings(domain: Option<&Domain>, key: &str) -> Vec<String> {
        item::<NSArray>(domain, key)
            .into_iter()
            .flat_map(|array| array.to_vec())
            .filter_map(|value| value.downcast::<NSString>().ok())
            .map(|value| value.to_string())
            .collect()
    }

    fn pins(domain: Option<&Domain>) -> BTreeMap<String, String> {
        let Some(pins) = item::<NSDictionary>(domain, "copied-md.pinnedFormats") else {
            return BTreeMap::new();
        };
        let (apps, targets) = pins.to_vecs();
        apps.into_iter()
            .zip(targets)
            .filter_map(|(id, target)| {
                let id = id.downcast::<NSString>().ok()?;
                let target = target.downcast::<NSString>().ok()?;
                Some((id.to_string(), target.to_string()))
            })
            .collect()
    }
}
