use copied_config::ShortcutSpec;

use crate::{
    ActiveApp, ActiveAppProvider, LoginItemBackend, LoginItemStatus, PlatformError, ShortcutBackend,
};

/// Deterministic register-first backend with injectable registration errors.
#[derive(Debug, Clone, Default)]
pub struct FakeShortcutBackend {
    current: Option<ShortcutSpec>,
    pub bind_error: Option<PlatformError>,
}

impl ShortcutBackend for FakeShortcutBackend {
    fn current(&self) -> Option<&ShortcutSpec> {
        self.current.as_ref()
    }

    fn bind(&mut self, shortcut: &ShortcutSpec) -> Result<(), PlatformError> {
        if self.current() == Some(shortcut) {
            return Ok(());
        }
        if let Some(error) = &self.bind_error {
            return Err(error.clone());
        }
        self.current = Some(shortcut.clone());
        Ok(())
    }

    fn unbind(&mut self) {
        self.current = None;
    }
}

/// In-memory foreground application identity.
#[derive(Debug, Clone, Default)]
pub struct FakeActiveAppProvider {
    pub active_app: Option<ActiveApp>,
}

impl ActiveAppProvider for FakeActiveAppProvider {
    fn frontmost(&self) -> Option<ActiveApp> {
        self.active_app.clone()
    }
}

/// In-memory launch-at-login registration with configurable approval and error states.
#[derive(Debug, Clone)]
pub struct FakeLoginItemBackend {
    pub current_status: LoginItemStatus,
    pub requires_approval: bool,
    pub set_error: Option<PlatformError>,
}

impl Default for FakeLoginItemBackend {
    fn default() -> Self {
        Self {
            current_status: LoginItemStatus::Disabled,
            requires_approval: false,
            set_error: None,
        }
    }
}

impl LoginItemBackend for FakeLoginItemBackend {
    fn status(&self) -> LoginItemStatus {
        self.current_status
    }

    fn set_enabled(&mut self, enabled: bool) -> Result<LoginItemStatus, PlatformError> {
        if let Some(error) = &self.set_error {
            return Err(error.clone());
        }
        if self.current_status == LoginItemStatus::Unsupported {
            return Err(PlatformError::Unsupported("launch at login".into()));
        }
        self.current_status = match (enabled, self.requires_approval) {
            (false, _) => LoginItemStatus::Disabled,
            (true, true) => LoginItemStatus::RequiresApproval,
            (true, false) => LoginItemStatus::Enabled,
        };
        Ok(self.status())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_replacement_preserves_previous_binding_on_error() {
        let mut backend = FakeShortcutBackend::default();
        let first = ShortcutSpec::parse("Ctrl+Shift+A").unwrap();
        let next = ShortcutSpec::parse("Ctrl+Shift+B").unwrap();
        assert_eq!(backend.current(), None);
        backend.bind(&first).unwrap();
        let conflict = PlatformError::ShortcutInUse(next.format_portable());
        backend.bind_error = Some(conflict.clone());
        backend.bind(&first).unwrap();
        assert_eq!(backend.bind(&next), Err(conflict));
        assert_eq!(backend.current(), Some(&first));
        backend.bind_error = None;
        backend.bind(&next).unwrap();
        assert_eq!(backend.current(), Some(&next));
        backend.unbind();
        assert_eq!(backend.current(), None);
    }

    #[test]
    fn foreground_identity_can_change_or_disappear() {
        let mut provider = FakeActiveAppProvider::default();
        assert_eq!(provider.frontmost(), None);
        let app = ActiveApp {
            id: "x11:wmclass:Test".into(),
            name: "Test".into(),
        };
        provider.active_app = Some(app.clone());
        assert_eq!(provider.frontmost(), Some(app));
        provider.active_app = None;
        assert_eq!(provider.frontmost(), None);
    }

    #[test]
    fn login_status_tracks_approval_and_write_errors() {
        let mut backend = FakeLoginItemBackend::default();
        assert_eq!(backend.status(), LoginItemStatus::Disabled);
        assert_eq!(backend.set_enabled(true).unwrap(), LoginItemStatus::Enabled);
        let failure = PlatformError::LoginItemFailed("rejected".into());
        backend.set_error = Some(failure.clone());
        assert_eq!(backend.set_enabled(false), Err(failure));
        assert_eq!(backend.status(), LoginItemStatus::Enabled);
        backend.set_error = None;
        assert_eq!(
            backend.set_enabled(false).unwrap(),
            LoginItemStatus::Disabled
        );
        backend.requires_approval = true;
        assert_eq!(
            backend.set_enabled(true).unwrap(),
            LoginItemStatus::RequiresApproval
        );
        backend.current_status = LoginItemStatus::Unsupported;
        assert!(matches!(
            backend.set_enabled(true),
            Err(PlatformError::Unsupported(_))
        ));
    }
}
