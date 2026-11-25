use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use copied_config::{ConfigPatch, ShortcutCopyMode, default_shortcut};
use copied_platform::{LoginItemStatus, ShortcutCapability};
use tauri::{
    AppHandle, Manager,
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use crate::{
    DesktopError, DesktopRuntime,
    desktop::{DesktopState, Feedback},
    picker::Panel,
    with_desktop,
};

const TRAY_ID: &str = "copied-md";
const NORMAL_ICON: &[u8] = include_bytes!("../icons/tray.png");
const COPIED_ICON: &[u8] = include_bytes!("../icons/tray-copied.png");
const MODES: [(&str, &str, ShortcutCopyMode); 3] = [
    (
        "copy-and-pick",
        "Copy Last Used and Show Picker",
        ShortcutCopyMode::CopyAndPick,
    ),
    ("picker", "Show Picker First", ShortcutCopyMode::Picker),
    (
        "copy-only",
        "Copy Last Used Only",
        ShortcutCopyMode::CopyOnly,
    ),
];

pub(crate) struct TrayMenu {
    enabled: CheckMenuItem<tauri::Wry>,
    recording: MenuItem<tauri::Wry>,
    pin: MenuItem<tauri::Wry>,
    reset: MenuItem<tauri::Wry>,
    modes: Vec<(ShortcutCopyMode, CheckMenuItem<tauri::Wry>)>,
    login: CheckMenuItem<tauri::Wry>,
    shortcut_failure: MenuItem<tauri::Wry>,
    flash_generation: Arc<AtomicU64>,
    menu: Menu<tauri::Wry>,
}

impl TrayMenu {
    pub(crate) fn install(app: &AppHandle) -> tauri::Result<Self> {
        let copy = MenuItem::with_id(app, "copy", "Copy Markdown As…", true, None::<&str>)?;
        let enabled = CheckMenuItem::with_id(
            app,
            "shortcut-enabled",
            "Use Keyboard Shortcut",
            true,
            false,
            None::<&str>,
        )?;
        let recording = MenuItem::with_id(
            app,
            "shortcut-record",
            "Change Keyboard Shortcut…",
            true,
            None::<&str>,
        )?;
        let behavior = Submenu::new(app, "Shortcut Behavior", true)?;
        let mut modes = Vec::with_capacity(MODES.len());
        for (id, title, mode) in MODES {
            let item = CheckMenuItem::with_id(app, id, title, true, false, None::<&str>)?;
            behavior.append(&item)?;
            modes.push((mode, item));
        }

        let pin = MenuItem::with_id(
            app,
            "pin",
            "Choose Format for Active App…",
            true,
            None::<&str>,
        )?;
        let reset = MenuItem::with_id(
            app,
            "shortcut-reset",
            "Reset to Default Shortcut",
            true,
            None::<&str>,
        )?;
        let login =
            CheckMenuItem::with_id(app, "login", "Launch at Login", true, false, None::<&str>)?;
        let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
        let about = MenuItem::with_id(app, "about", "About copied.md", true, None::<&str>)?;
        let quit = MenuItem::with_id(app, "quit", "Quit copied.md", true, None::<&str>)?;
        let shortcut_failure = MenuItem::new(app, "", false, None::<&str>)?;
        let menu = Menu::with_items(
            app,
            &[
                &copy,
                &PredefinedMenuItem::separator(app)?,
                &behavior,
                &enabled,
                &recording,
                &reset,
                &pin,
                &PredefinedMenuItem::separator(app)?,
                &settings,
                &login,
                &about,
                &PredefinedMenuItem::separator(app)?,
                &quit,
            ],
        )?;
        TrayIconBuilder::with_id(TRAY_ID)
            .icon(Image::from_bytes(NORMAL_ICON)?)
            .icon_as_template(true)
            .tooltip("copied.md")
            .menu(&menu)
            .on_menu_event(|app, event| handle_menu(app, event.id().as_ref()))
            .on_tray_icon_event(|tray, event| {
                if matches!(
                    event,
                    TrayIconEvent::Enter { .. } | TrayIconEvent::Click { .. }
                ) {
                    let _ = with_desktop(tray.app_handle(), |_| Ok(()));
                }
            })
            .build(app)?;
        Ok(Self {
            enabled,
            recording,
            pin,
            reset,
            modes,
            login,
            shortcut_failure,
            flash_generation: Arc::default(),
            menu,
        })
    }

    pub(crate) fn refresh(&self, state: &DesktopState) -> tauri::Result<()> {
        let direct = state.capabilities.global_shortcuts == ShortcutCapability::Direct;
        self.enabled.set_enabled(direct)?;
        self.enabled.set_checked(state.shortcut_registered)?;
        self.recording.set_enabled(direct)?;
        self.reset.set_enabled(
            direct && (state.config.shortcut != default_shortcut() || !state.shortcut_registered),
        )?;
        let failure = state.shortcut_failure.as_deref();
        self.shortcut_failure.set_text(failure.unwrap_or(""))?;
        // Menu items lack a portable visibility switch. Insert or remove the status row.
        let inserted = self.menu.get(self.shortcut_failure.id()).is_some();
        if failure.is_some() && !inserted {
            self.menu.insert(&self.shortcut_failure, 6)?;
        } else if failure.is_none() && inserted {
            self.menu.remove(&self.shortcut_failure)?;
        }
        for (mode, item) in &self.modes {
            item.set_checked(state.config.shortcut_mode == *mode)?;
        }
        self.pin
            .set_enabled(state.capabilities.active_app_detection)?;
        let pin = if cfg!(target_os = "linux") {
            // Linux tray integrations do not consistently send mouse events before opening.
            "Choose Format for Active App…".into()
        } else {
            state.active_app.as_ref().map_or_else(
                || "Choose Format for Active App…".into(),
                |app| format!("Choose Format for {}…", app.name),
            )
        };
        self.pin.set_text(pin)?;
        self.login.set_enabled(state.capabilities.launch_at_login)?;
        self.login.set_checked(matches!(
            state.login_item_status,
            LoginItemStatus::Enabled | LoginItemStatus::RequiresApproval
        ))?;
        self.login.set_text(
            if state.login_item_status == LoginItemStatus::RequiresApproval {
                "Launch at Login (approval required)"
            } else {
                "Launch at Login"
            },
        )?;
        Ok(())
    }

    pub(crate) fn present(&self, app: &AppHandle, feedback: Vec<Feedback>) {
        for event in feedback {
            match event {
                Feedback::Copied(target) => self.flash(app, target.definition().label.as_ref()),
                Feedback::Warning(title, detail) => {
                    dialog(app, &title, &detail, MessageDialogKind::Warning)
                }
                Feedback::Error(error) => dialog(
                    app,
                    "Could not copy Markdown",
                    &error,
                    MessageDialogKind::Error,
                ),
            }
        }
    }

    fn flash(&self, app: &AppHandle, label: &str) {
        let Some(tray) = app.tray_by_id(TRAY_ID) else {
            return;
        };
        let _ = tray.set_icon(Image::from_bytes(COPIED_ICON).ok());
        let _ = tray.set_tooltip(Some(format!("Copied as {label}")));
        let generation = Arc::clone(&self.flash_generation);
        let flashed = generation.fetch_add(1, Ordering::Relaxed) + 1;
        tauri::async_runtime::spawn_blocking(move || {
            std::thread::sleep(Duration::from_millis(900));
            let app = tray.app_handle().clone();
            let _ = app.run_on_main_thread(move || {
                if generation.load(Ordering::Relaxed) == flashed {
                    let _ = tray.set_icon(Image::from_bytes(NORMAL_ICON).ok());
                    let _ = tray.set_tooltip(Some("copied.md"));
                }
            });
        });
    }
}

fn handle_menu(app: &AppHandle, id: &str) {
    if id == "quit" {
        app.state::<DesktopRuntime>()
            .quitting
            .store(true, Ordering::Relaxed);
        app.exit(0);
        return;
    }
    if id == "about" {
        let result = with_desktop(app, |desktop| Ok(desktop.state()));
        let shortcut = result
            .ok()
            .filter(|state| state.shortcut_registered)
            .map(|state| {
                if cfg!(target_os = "macos") {
                    state.config.shortcut.format_macos()
                } else {
                    state.config.shortcut.format_portable()
                }
            });
        let message = shortcut.map_or_else(
            || "Copy Markdown in any app, then choose Copy Markdown As… from this menu.".into(),
            |shortcut| format!("Copy Markdown in any app, then press {shortcut} or choose Copy Markdown As… from this menu."),
        );
        dialog(
            app,
            &format!("About copied.md · {}", app.package_info().version),
            &message,
            MessageDialogKind::Info,
        );
        return;
    }

    let result = with_desktop(app, |desktop| {
        match id {
            "copy" => desktop.start(Some(ShortcutCopyMode::Picker))?,
            "settings" => desktop.show_panel(Panel::Settings)?,
            "pin" => desktop.show_panel(Panel::AppPin)?,
            "shortcut-record" => desktop.show_panel(Panel::Shortcut)?,
            "shortcut-reset" => {
                desktop.reset_shortcut()?;
            }
            "shortcut-enabled" => {
                desktop.toggle_shortcut(!desktop.state().shortcut_registered)?;
            }
            "login" => {
                let enable = desktop.state().login_item_status == LoginItemStatus::Disabled;
                if desktop.toggle_login(enable)? == LoginItemStatus::RequiresApproval {
                    desktop.engine.notifier.0.push(Feedback::Warning(
                        "Approve Launch at Login".into(),
                        "Allow copied.md to launch at login in your system settings.".into(),
                    ));
                }
            }
            id => {
                if let Some((_, _, mode)) = MODES.iter().find(|(key, _, _)| *key == id) {
                    desktop.update_config(ConfigPatch {
                        shortcut_mode: Some(*mode),
                        ..ConfigPatch::default()
                    })?;
                }
            }
        }
        Ok(())
    });
    present_operation_error(app, result);
}

pub(crate) fn present_operation_error(app: &AppHandle, result: Result<(), DesktopError>) {
    if let Err(error) = result
        && !matches!(
            error,
            DesktopError::Engine(copied_engine::EngineError::Clipboard(_))
        )
    {
        dialog(
            app,
            "copied.md",
            &error.to_string(),
            MessageDialogKind::Error,
        );
    }
}

fn dialog(app: &AppHandle, title: &str, detail: &str, kind: MessageDialogKind) {
    app.dialog()
        .message(detail)
        .title(title)
        .kind(kind)
        .show(|_| {});
}
