use copied_clipboard::{Board, ClipboardSnapshot, Revision, WriteError, WrittenClipboard};
use copied_config::ShortcutSpec;
use copied_core::ClipboardPayload;
use copied_platform::{
    ActiveApp, ActiveAppProvider, LoginItemBackend, PlatformError, ShortcutBackend,
    ShortcutCapability,
};

pub(crate) struct DesktopBoard(Result<Box<dyn Board>, WriteError>);

impl DesktopBoard {
    pub(crate) fn system() -> Self {
        Self(copied_clipboard::system_board())
    }
}

impl Board for DesktopBoard {
    fn revision(&self) -> Result<Revision, WriteError> {
        self.0.as_ref().map_err(Clone::clone)?.revision()
    }

    fn read_text(&self) -> Result<ClipboardSnapshot, WriteError> {
        self.0.as_ref().map_err(Clone::clone)?.read_text()
    }

    fn write(
        &mut self,
        payload: &ClipboardPayload,
        expected: Option<Revision>,
    ) -> Result<WrittenClipboard, WriteError> {
        self.0
            .as_mut()
            .map_err(|error| error.clone())?
            .write(payload, expected)
    }
}

pub(crate) struct DesktopShortcut(Result<Box<dyn ShortcutBackend>, PlatformError>);

impl DesktopShortcut {
    pub(crate) fn new(
        capability: ShortcutCapability,
        callback: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self(match capability {
            ShortcutCapability::Direct => direct_shortcut(callback),
            ShortcutCapability::Portal => Err(PlatformError::Unsupported(
                "Use your desktop's keyboard shortcut settings to manage Wayland shortcuts.".into(),
            )),
            ShortcutCapability::Unsupported => Err(PlatformError::Unsupported(
                "This desktop session does not support global shortcuts.".into(),
            )),
        })
    }
}

impl ShortcutBackend for DesktopShortcut {
    fn current(&self) -> Option<&ShortcutSpec> {
        self.0.as_ref().ok()?.current()
    }

    fn bind(&mut self, spec: &ShortcutSpec) -> Result<(), PlatformError> {
        self.0.as_mut().map_err(|error| error.clone())?.bind(spec)
    }

    fn unbind(&mut self) {
        if let Ok(backend) = &mut self.0 {
            backend.unbind();
        }
    }
}

fn direct_shortcut(
    callback: impl Fn() + Send + Sync + 'static,
) -> Result<Box<dyn ShortcutBackend>, PlatformError> {
    #[cfg(target_os = "macos")]
    return Ok(Box::new(copied_platform::macos::MacHotKey::new(callback)?));
    #[cfg(target_os = "windows")]
    return Ok(Box::new(copied_platform::windows::WinHotKey::new(
        callback,
    )?));
    #[cfg(target_os = "linux")]
    return Ok(Box::new(copied_platform::linux::X11HotKey::new(callback)?));
}

pub(crate) fn active_app_provider() -> Option<Box<dyn ActiveAppProvider>> {
    #[cfg(target_os = "macos")]
    return Some(Box::new(copied_platform::macos::MacActiveApp));
    #[cfg(target_os = "windows")]
    return Some(Box::new(copied_platform::windows::WinActiveApp));
    #[cfg(target_os = "linux")]
    return copied_platform::linux::X11ActiveApp::connect()
        .ok()
        .map(|provider| Box::new(provider) as _);
}

pub(crate) fn login_item_backend() -> Option<Box<dyn LoginItemBackend>> {
    #[cfg(target_os = "macos")]
    return Some(Box::new(copied_platform::macos::MacLoginItem));
    #[cfg(target_os = "windows")]
    return Some(Box::new(copied_platform::windows::WinLoginItem));
    #[cfg(target_os = "linux")]
    return copied_platform::linux::XdgLoginItem::new()
        .ok()
        .map(|backend| Box::new(backend) as _);
}

pub(crate) fn frontmost(provider: &Option<Box<dyn ActiveAppProvider>>) -> Option<ActiveApp> {
    provider.as_ref()?.frontmost()
}

#[cfg(test)]
impl DesktopBoard {
    pub(crate) fn from_board(board: impl Board + 'static) -> Self {
        Self(Ok(Box::new(board)))
    }
}

#[cfg(test)]
impl DesktopShortcut {
    pub(crate) fn from_backend(backend: impl ShortcutBackend + 'static) -> Self {
        Self(Ok(Box::new(backend)))
    }
}
