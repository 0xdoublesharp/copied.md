use std::path::Path;

use objc2::rc::autoreleasepool;
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{
    NSArray, NSBundle, NSNumber, NSOperatingSystemVersion, NSProcessInfo, NSString, NSURL,
    NSURLVolumeIsReadOnlyKey,
};
use objc2_service_management::{SMAppService, SMAppServiceStatus};

use crate::{
    ActiveApp, ActiveAppProvider, LoginItemBackend, LoginItemStatus, PlatformCapabilities,
    PlatformError, ShortcutCapability,
};

mod hotkey;
pub use hotkey::MacHotKey;

pub(crate) fn capabilities() -> PlatformCapabilities {
    PlatformCapabilities {
        background_clipboard: true,
        global_shortcuts: ShortcutCapability::Direct,
        active_app_detection: true,
        launch_at_login: MacLoginItem.status() != LoginItemStatus::Unsupported
            && can_register_bundle(&main_bundle_path()),
    }
}

/// macOS frontmost application lookup, with the current process filtered out.
#[derive(Debug, Default)]
pub struct MacActiveApp;

impl ActiveAppProvider for MacActiveApp {
    fn frontmost(&self) -> Option<ActiveApp> {
        autoreleasepool(|_| {
            let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
            let bundle = app.bundleIdentifier()?;
            let own_bundle = NSBundle::mainBundle().bundleIdentifier();
            if app.processIdentifier() == i32::try_from(std::process::id()).ok()?
                || own_bundle.as_deref() == Some(&bundle)
            {
                return None;
            }
            let bundle = bundle.to_string();
            Some(ActiveApp {
                id: format!("mac:bundle:{bundle}"),
                name: app.localizedName().map_or(bundle, |name| name.to_string()),
            })
        })
    }
}

/// Launch-at-login registration for the application's main bundle.
#[derive(Debug, Default)]
pub struct MacLoginItem;

fn login_service_available() -> bool {
    autoreleasepool(|_| {
        // ServiceManagement introduced SMAppService in macOS 13.
        NSProcessInfo::processInfo().isOperatingSystemAtLeastVersion(NSOperatingSystemVersion {
            majorVersion: 13,
            minorVersion: 0,
            patchVersion: 0,
        })
    })
}

fn main_bundle_path() -> std::path::PathBuf {
    autoreleasepool(|_| NSBundle::mainBundle().bundlePath().to_string().into())
}

/// Registration is disabled for translocated bundles and read-only volumes.
#[allow(unsafe_code)]
pub fn can_register_bundle(path: &Path) -> bool {
    let path = path.to_string_lossy();
    !path.contains("/AppTranslocation/")
        && autoreleasepool(|_| {
            let url = NSURL::fileURLWithPath(&NSString::from_str(&path));
            // SAFETY: Foundation supplies this immutable resource-key string.
            let key = unsafe { NSURLVolumeIsReadOnlyKey };
            !url.resourceValuesForKeys_error(&NSArray::from_slice(&[key]))
                .ok()
                .and_then(|values| values.objectForKey(key))
                .and_then(|value| value.downcast::<NSNumber>().ok())
                .is_some_and(|value| value.boolValue())
        })
}

fn login_status(status: SMAppServiceStatus) -> LoginItemStatus {
    match status {
        SMAppServiceStatus::NotRegistered => LoginItemStatus::Disabled,
        SMAppServiceStatus::Enabled => LoginItemStatus::Enabled,
        SMAppServiceStatus::RequiresApproval => LoginItemStatus::RequiresApproval,
        _ => LoginItemStatus::Unsupported,
    }
}

impl LoginItemBackend for MacLoginItem {
    #[allow(unsafe_code)]
    fn status(&self) -> LoginItemStatus {
        if !login_service_available() {
            return LoginItemStatus::Unsupported;
        }
        // SAFETY: The OS availability check above guarantees these selectors exist.
        autoreleasepool(|_| login_status(unsafe { SMAppService::mainAppService().status() }))
    }

    #[allow(unsafe_code)]
    fn set_enabled(&mut self, enabled: bool) -> Result<LoginItemStatus, PlatformError> {
        if !login_service_available() {
            return Err(PlatformError::Unsupported(
                "SMAppService requires macOS 13".into(),
            ));
        }
        let status = self.status();
        if matches!(
            (enabled, status),
            (
                true,
                LoginItemStatus::Enabled | LoginItemStatus::RequiresApproval
            ) | (false, LoginItemStatus::Disabled)
        ) {
            return Ok(status);
        }
        if enabled && !can_register_bundle(&main_bundle_path()) {
            return Err(PlatformError::LoginItemFailed(
                "Move copied.md to Applications before enabling Launch at Login.".into(),
            ));
        }

        autoreleasepool(|_| {
            // SAFETY: These selectors are available on macOS 13 and use the main bundle's service.
            let service = unsafe { SMAppService::mainAppService() };
            // SAFETY: The service is live and these calls use objc2's checked NSError convention.
            unsafe {
                if enabled {
                    service.registerAndReturnError()
                } else {
                    service.unregisterAndReturnError()
                }
            }
            .map_err(|error| {
                PlatformError::LoginItemFailed(error.localizedDescription().to_string())
            })?;
            // SAFETY: The service remains live and the selector is available.
            Ok(login_status(unsafe { service.status() }))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translocated_bundle_is_ineligible_for_registration() {
        assert!(!can_register_bundle(Path::new(
            "/private/AppTranslocation/Instance/copied.md.app"
        )));
        assert!(can_register_bundle(&main_bundle_path()));
    }

    #[test]
    fn capabilities_match_this_macos_installation() {
        let actual = crate::detect_capabilities();
        assert!(actual.background_clipboard && actual.active_app_detection);
        assert_eq!(actual.global_shortcuts, ShortcutCapability::Direct);
        assert_eq!(
            actual.launch_at_login,
            MacLoginItem.status() != LoginItemStatus::Unsupported
        );
        // Querying the system does not register a login item.
        let _ = MacLoginItem.status();
        let _ = MacActiveApp.frontmost();
    }

    #[test]
    fn login_status_maps_approval_and_missing_service() {
        for (native, expected) in [
            (SMAppServiceStatus::NotRegistered, LoginItemStatus::Disabled),
            (SMAppServiceStatus::Enabled, LoginItemStatus::Enabled),
            (
                SMAppServiceStatus::RequiresApproval,
                LoginItemStatus::RequiresApproval,
            ),
            (SMAppServiceStatus::NotFound, LoginItemStatus::Unsupported),
        ] {
            assert_eq!(login_status(native), expected);
        }
    }
}
