use std::{os::windows::ffi::OsStrExt, path::PathBuf};

use windows::{
    Win32::{
        Foundation::{
            CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS, HANDLE,
        },
        System::{
            Registry::{
                HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
                RRF_RT_REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegGetValueW,
                RegOpenKeyExW, RegSetValueExW,
            },
            StationsAndDesktops::{
                CloseDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS, OpenInputDesktop,
            },
            Threading::{
                OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            },
        },
        UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
    },
    core::{PCWSTR, PWSTR, w},
};

use crate::{
    ActiveApp, ActiveAppProvider, LoginItemBackend, LoginItemStatus, PlatformCapabilities,
    PlatformError, ShortcutCapability,
};

mod hotkey;
pub use hotkey::WinHotKey;

const RUN: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE: PCWSTR = w!("copied.md");

#[allow(unsafe_code)]
pub(crate) fn capabilities() -> PlatformCapabilities {
    // SAFETY: Probe the current input desktop, and release the returned handle immediately.
    let interactive = unsafe {
        OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS).is_ok_and(
            |desktop| {
                let _ = CloseDesktop(desktop);
                true
            },
        )
    };
    PlatformCapabilities {
        background_clipboard: interactive,
        global_shortcuts: if interactive {
            ShortcutCapability::Direct
        } else {
            ShortcutCapability::Unsupported
        },
        active_app_detection: interactive,
        launch_at_login: WinLoginItem.status() != LoginItemStatus::Unsupported,
    }
}

struct Process(HANDLE);

impl Drop for Process {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: Our OpenProcess handle is owned by this guard.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

#[derive(Debug, Default)]
pub struct WinActiveApp;

impl ActiveAppProvider for WinActiveApp {
    #[allow(unsafe_code)]
    fn frontmost(&self) -> Option<ActiveApp> {
        // SAFETY: The window handle is queried only for its PID; destruction is handled as failure.
        let foreground = unsafe { GetForegroundWindow() };
        if foreground.is_invalid() {
            return None;
        }
        let mut pid = 0;
        // SAFETY: pid is a valid DWORD out-parameter.
        unsafe { GetWindowThreadProcessId(foreground, Some(&raw mut pid)) };
        if pid == 0 || pid == std::process::id() {
            return None;
        }
        // SAFETY: Request a non-inheritable query-only process handle.
        let process =
            Process(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?);
        let mut length = 32_768_u32;
        let mut path = vec![0; length as usize];
        // SAFETY: path contains length UTF-16 units and remains live throughout the query.
        unsafe {
            QueryFullProcessImageNameW(
                process.0,
                PROCESS_NAME_WIN32,
                PWSTR(path.as_mut_ptr()),
                &raw mut length,
            )
        }
        .ok()?;
        let path = PathBuf::from(String::from_utf16(path.get(..length as usize)?).ok()?);
        let name = path.file_name()?.to_str()?.to_owned();
        Some(ActiveApp {
            id: format!("win:exe:{}", name.to_lowercase()),
            name,
        })
    }
}

struct RunKey(HKEY);

impl Drop for RunKey {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: This is an owned registry key returned by a successful open.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

/// Launch-at-login registration in the current user's Run key.
#[derive(Debug, Default)]
pub struct WinLoginItem;

impl LoginItemBackend for WinLoginItem {
    #[allow(unsafe_code)]
    fn status(&self) -> LoginItemStatus {
        let mut size = 0;
        // SAFETY: With no output buffer, RegGetValueW only reports the named value's size.
        let result = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                RUN,
                VALUE,
                RRF_RT_REG_SZ,
                None,
                None,
                Some(&raw mut size),
            )
        };
        match result {
            ERROR_SUCCESS => LoginItemStatus::Enabled,
            ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => LoginItemStatus::Disabled,
            _ => LoginItemStatus::Unsupported,
        }
    }

    #[allow(unsafe_code)]
    fn set_enabled(&mut self, enabled: bool) -> Result<LoginItemStatus, PlatformError> {
        let mut key = HKEY::default();
        // SAFETY: The paths are static NUL-terminated strings and key is initialized on success.
        let opened = unsafe {
            if enabled {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    RUN,
                    None,
                    PCWSTR::null(),
                    REG_OPTION_NON_VOLATILE,
                    KEY_SET_VALUE,
                    None,
                    &raw mut key,
                    None,
                )
            } else {
                RegOpenKeyExW(HKEY_CURRENT_USER, RUN, None, KEY_SET_VALUE, &raw mut key)
            }
        };
        if !enabled && matches!(opened, ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND) {
            return Ok(LoginItemStatus::Disabled);
        }
        opened
            .ok()
            .map_err(|error| PlatformError::LoginItemFailed(error.to_string()))?;
        let key = RunKey(key);
        let status = if enabled {
            let executable = std::env::current_exe()
                .map_err(|error| PlatformError::LoginItemFailed(error.to_string()))?;
            let quote = u16::from(b'"');
            let bytes: Vec<u8> = [quote]
                .into_iter()
                .chain(executable.as_os_str().encode_wide())
                .chain([quote, 0])
                .flat_map(u16::to_le_bytes)
                .collect();
            // SAFETY: key is open for writing; the command is NUL-terminated UTF-16.
            unsafe { RegSetValueExW(key.0, VALUE, None, REG_SZ, Some(&bytes)) }
        } else {
            // SAFETY: key is open for writing; VALUE is an immutable NUL-terminated string.
            let status = unsafe { RegDeleteValueW(key.0, VALUE) };
            if status == ERROR_FILE_NOT_FOUND {
                return Ok(LoginItemStatus::Disabled);
            }
            status
        };
        status
            .ok()
            .map_err(|error| PlatformError::LoginItemFailed(error.to_string()))?;
        Ok(if enabled {
            LoginItemStatus::Enabled
        } else {
            LoginItemStatus::Disabled
        })
    }
}
