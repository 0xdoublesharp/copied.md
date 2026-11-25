use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};

use copied_clipboard::linux::{Session, background_clipboard_available, desktop_session};
use x11rb::{
    connection::Connection,
    protocol::xproto::{Atom, AtomEnum, ConnectionExt, Window},
    rust_connection::RustConnection,
};

use crate::{
    ActiveApp, ActiveAppProvider, LoginItemBackend, LoginItemStatus, PlatformCapabilities,
    PlatformError, ShortcutCapability,
};

mod hotkey;
pub use hotkey::X11HotKey;

pub(crate) fn capabilities() -> PlatformCapabilities {
    let session = desktop_session();
    let shortcuts = match session {
        Session::X11 if x11rb::connect(None).is_ok() => ShortcutCapability::Direct,
        Session::Wayland if shortcut_portal_available() => ShortcutCapability::Portal,
        _ => ShortcutCapability::Unsupported,
    };
    capability_matrix(
        session,
        shortcuts,
        background_clipboard_available(),
        config_home().is_some(),
    )
}

fn capability_matrix(
    session: Session,
    global_shortcuts: ShortcutCapability,
    clipboard: bool,
    autostart: bool,
) -> PlatformCapabilities {
    PlatformCapabilities {
        background_clipboard: clipboard,
        global_shortcuts,
        active_app_detection: session == Session::X11
            && global_shortcuts == ShortcutCapability::Direct,
        launch_at_login: session != Session::Headless && autostart,
    }
}

fn shortcut_portal_available() -> bool {
    // Check the session service, since a compositor alone does not supply the portal.
    Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--timeout",
            "2",
            "--dest",
            "org.freedesktop.portal.Desktop",
            "--object-path",
            "/org/freedesktop/portal/desktop",
            "--method",
            "org.freedesktop.DBus.Properties.Get",
            "org.freedesktop.portal.GlobalShortcuts",
            "version",
        ])
        .output()
        .is_ok_and(|output| output.status.success())
}

#[derive(Debug)]
pub struct X11ActiveApp {
    connection: RustConnection,
    root: Window,
    active_window: Atom,
    pid: Atom,
}

impl X11ActiveApp {
    pub fn connect() -> Result<Self, PlatformError> {
        if desktop_session() != Session::X11 {
            return Err(PlatformError::Unsupported(
                "foreground application lookup requires X11".into(),
            ));
        }
        let (connection, screen) =
            x11rb::connect(None).map_err(|error| PlatformError::Unsupported(error.to_string()))?;
        let root = connection.setup().roots[screen].root;
        let active_window = intern(&connection, b"_NET_ACTIVE_WINDOW")?;
        let pid = intern(&connection, b"_NET_WM_PID")?;
        Ok(Self {
            connection,
            root,
            active_window,
            pid,
        })
    }
}

fn intern(connection: &RustConnection, name: &[u8]) -> Result<Atom, PlatformError> {
    connection
        .intern_atom(false, name)
        .map_err(|error| PlatformError::Unsupported(error.to_string()))?
        .reply()
        .map(|reply| reply.atom)
        .map_err(|error| PlatformError::Unsupported(error.to_string()))
}

impl ActiveAppProvider for X11ActiveApp {
    fn frontmost(&self) -> Option<ActiveApp> {
        let window = self
            .connection
            .get_property(false, self.root, self.active_window, AtomEnum::WINDOW, 0, 1)
            .ok()?
            .reply()
            .ok()?
            .value32()?
            .next()
            .filter(|window| *window != x11rb::NONE)?;
        let own_window = self
            .connection
            .get_property(false, window, self.pid, AtomEnum::CARDINAL, 0, 1)
            .ok()?
            .reply()
            .ok()?
            .value32()
            .and_then(|mut values| values.next())
            == Some(std::process::id());
        if own_window {
            return None;
        }
        let class = self
            .connection
            .get_property(
                false,
                window,
                AtomEnum::WM_CLASS,
                AtomEnum::STRING,
                0,
                1_024,
            )
            .ok()?
            .reply()
            .ok()?;
        if class.bytes_after != 0 || class.format != 8 {
            return None;
        }
        active_app(&class.value)
    }
}

fn active_app(class_property: &[u8]) -> Option<ActiveApp> {
    let class = class_property
        .split(|byte| *byte == 0)
        .nth(1)?
        .iter()
        .copied()
        .map(char::from)
        .collect::<String>();
    (!class.is_empty()).then(|| ActiveApp {
        id: format!("x11:wmclass:{class}"),
        name: class,
    })
}

fn config_home() -> Option<PathBuf> {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|home| home.join(".config"))
        })
}

/// The per-user XDG autostart entry for copied.md.
#[derive(Debug)]
pub struct XdgLoginItem {
    path: PathBuf,
    executable: PathBuf,
}

impl XdgLoginItem {
    pub fn new() -> Result<Self, PlatformError> {
        let config = config_home()
            .ok_or_else(|| PlatformError::Unsupported("no XDG configuration directory".into()))?;
        let executable = env::current_exe().map_err(login_error)?;
        Ok(Self {
            path: config.join("autostart/md.copied.desktop"),
            executable,
        })
    }
}

fn login_error(error: impl std::fmt::Display) -> PlatformError {
    PlatformError::LoginItemFailed(error.to_string())
}

fn autostart_entry(executable: &Path) -> Result<String, PlatformError> {
    let executable = executable
        .to_str()
        .filter(|path| !path.chars().any(char::is_control))
        .ok_or_else(|| {
            PlatformError::LoginItemFailed("invalid desktop entry executable path".into())
        })?;
    // Exec quoting is applied after desktop-entry value unescaping.
    let mut quoted = String::new();
    for character in executable.chars() {
        match character {
            '\\' => quoted.push_str(r"\\\\"),
            '"' | '`' | '$' => {
                quoted.push_str(r"\\");
                quoted.push(character);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(character),
        }
    }
    Ok(format!(
        "[Desktop Entry]\nType=Application\nName=copied.md\nExec=\"{quoted}\"\n"
    ))
}

impl LoginItemBackend for XdgLoginItem {
    fn status(&self) -> LoginItemStatus {
        match fs::read_to_string(&self.path) {
            Ok(entry) if entry.lines().any(|line| line.trim() == "Hidden=true") => {
                LoginItemStatus::Disabled
            }
            Ok(_) => LoginItemStatus::Enabled,
            Err(error) if error.kind() == io::ErrorKind::NotFound => LoginItemStatus::Disabled,
            Err(_) => LoginItemStatus::Unsupported,
        }
    }

    fn set_enabled(&mut self, enabled: bool) -> Result<LoginItemStatus, PlatformError> {
        if !enabled {
            match fs::remove_file(&self.path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(login_error(error)),
            }
            return Ok(LoginItemStatus::Disabled);
        }
        let entry = autostart_entry(&self.executable)?;
        let directory = self.path.parent().expect("autostart entry has a parent");
        fs::create_dir_all(directory).map_err(login_error)?;
        let mut temporary = tempfile::NamedTempFile::new_in(directory).map_err(login_error)?;
        temporary.write_all(entry.as_bytes()).map_err(login_error)?;
        temporary
            .persist(&self.path)
            .map_err(|error| login_error(error.error))?;
        Ok(LoginItemStatus::Enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wayland_does_not_inherit_xwayland_app_or_hotkey_capabilities() {
        assert_eq!(
            capability_matrix(Session::Wayland, ShortcutCapability::Portal, false, true),
            PlatformCapabilities {
                background_clipboard: false,
                global_shortcuts: ShortcutCapability::Portal,
                active_app_detection: false,
                launch_at_login: true
            }
        );
        assert_eq!(
            capability_matrix(
                Session::Wayland,
                ShortcutCapability::Unsupported,
                true,
                true
            )
            .global_shortcuts,
            ShortcutCapability::Unsupported
        );
        assert_eq!(
            capability_matrix(Session::X11, ShortcutCapability::Direct, true, true)
                .global_shortcuts,
            ShortcutCapability::Direct
        );
        assert!(
            !capability_matrix(
                Session::Headless,
                ShortcutCapability::Unsupported,
                false,
                true
            )
            .launch_at_login
        );
    }

    #[test]
    fn wm_class_uses_the_class_name() {
        assert_eq!(
            active_app(b"xterm\0XTerm\0"),
            Some(ActiveApp {
                id: "x11:wmclass:XTerm".into(),
                name: "XTerm".into()
            })
        );
        assert_eq!(active_app(b"xterm\0\0"), None);
    }

    #[test]
    fn manages_only_its_own_autostart_file() {
        let temp = tempfile::tempdir().unwrap();
        let mut login = XdgLoginItem {
            path: temp.path().join("autostart/md.copied.desktop"),
            executable: "/opt/App Name/copied.md".into(),
        };
        assert_eq!(login.status(), LoginItemStatus::Disabled);
        assert_eq!(login.set_enabled(true).unwrap(), LoginItemStatus::Enabled);
        assert_eq!(login.status(), LoginItemStatus::Enabled);
        assert!(
            fs::read_to_string(&login.path)
                .unwrap()
                .contains("Exec=\"/opt/App Name/copied.md\"")
        );
        fs::write(&login.path, "[Desktop Entry]\nHidden=true\n").unwrap();
        assert_eq!(login.status(), LoginItemStatus::Disabled);
        assert_eq!(login.set_enabled(false).unwrap(), LoginItemStatus::Disabled);
        assert!(!login.path.exists());
        assert_eq!(login.set_enabled(false).unwrap(), LoginItemStatus::Disabled);
    }
}
