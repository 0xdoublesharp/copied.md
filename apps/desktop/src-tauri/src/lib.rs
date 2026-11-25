use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use copied_engine::CopyEngine;
use tauri::{AppHandle, Manager};
use thiserror::Error;

mod commands;
mod config;
mod desktop;
mod picker;
mod platform;
mod self_test;
mod tray;

use desktop::{Desktop, DesktopNotifier, Feedback};
use picker::{DesktopPicker, TauriWindowFactory};
use platform::{DesktopBoard, DesktopShortcut};
use tray::TrayMenu;

pub use self_test::self_test;

#[derive(Debug, Error)]
pub enum DesktopError {
    #[error("Could not locate the configuration directory.")]
    MissingConfigPath,
    #[error("The desktop state is unavailable.")]
    Unavailable,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Config(#[from] copied_config::ConfigError),
    #[error(transparent)]
    Shortcut(#[from] copied_engine::ShortcutError),
    #[error(transparent)]
    Platform(#[from] copied_platform::PlatformError),
    #[error(transparent)]
    Engine(#[from] copied_engine::EngineError),
    #[error(transparent)]
    Clipboard(#[from] copied_clipboard::WriteError),
    #[error(transparent)]
    Native(#[from] tauri::Error),
    #[error(
        "Could not save the shortcut: {save}. Could not restore the previous binding: {error}."
    )]
    ShortcutRestore {
        save: copied_config::ConfigError,
        error: copied_engine::ShortcutError,
    },
    #[error("Self-test failed: {0}")]
    SelfTest(&'static str),
}

struct DesktopRuntime {
    desktop: Mutex<Desktop>,
    quitting: AtomicBool,
}

fn with_desktop<T>(
    app: &AppHandle,
    action: impl FnOnce(&mut Desktop) -> Result<T, DesktopError>,
) -> Result<T, DesktopError> {
    let runtime = app
        .try_state::<DesktopRuntime>()
        .ok_or(DesktopError::Unavailable)?;
    let (result, state, feedback) = {
        let mut desktop = runtime
            .desktop
            .lock()
            .map_err(|_| DesktopError::Unavailable)?;
        let result = action(&mut desktop);
        if let Some(error) = desktop.engine.picker.failure.take() {
            desktop
                .engine
                .notifier
                .0
                .push(Feedback::Error(error.to_string()));
        }
        (
            result,
            desktop.state(),
            std::mem::take(&mut desktop.engine.notifier.0),
        )
    };

    if let Some(menu) = app.try_state::<TrayMenu>() {
        if let Err(error) = menu.refresh(&state) {
            eprintln!("copied.md: {error}");
        }
        menu.present(app, feedback);
    }
    result
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    let (path, config) = config::open_config()?;
    let mut capabilities = copied_platform::detect_capabilities();
    let handle = app.handle().clone();
    let shortcut = DesktopShortcut::new(capabilities.global_shortcuts, move || {
        let app = handle.clone();
        let _ = handle.run_on_main_thread(move || {
            let result = with_desktop(&app, |desktop| desktop.start(None));
            tray::present_operation_error(&app, result);
        });
    });
    let active_app = platform::active_app_provider();
    let login = platform::login_item_backend();
    capabilities.active_app_detection &= active_app.is_some();
    capabilities.launch_at_login &= login.is_some();
    let engine = CopyEngine::new(
        DesktopBoard::system(),
        DesktopPicker::new(TauriWindowFactory::new(app.handle().clone())),
        DesktopNotifier::default(),
        config,
    );
    let desktop = Desktop::new(engine, path, shortcut, capabilities, active_app, login);
    app.manage(DesktopRuntime {
        desktop: Mutex::new(desktop),
        quitting: AtomicBool::new(false),
    });
    app.manage(TrayMenu::install(app.handle())?);
    with_desktop(app.handle(), |_| Ok(()))?;
    Ok(())
}

pub fn run() -> Result<(), DesktopError> {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(setup)
        .on_window_event(picker::handle_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::get_desktop_state,
            commands::picker_choose,
            commands::picker_dismiss,
            commands::close_panel,
            commands::update_config,
            commands::set_app_pin,
            commands::begin_shortcut_recording,
            commands::commit_shortcut_recording,
            commands::cancel_shortcut_recording,
            commands::reset_shortcut,
            commands::toggle_shortcut,
            commands::toggle_login_item,
            commands::preview_markdown,
        ])
        .build(tauri::generate_context!())?
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event
                && app
                    .try_state::<DesktopRuntime>()
                    .is_none_or(|state| !state.quitting.load(Ordering::Relaxed))
            {
                api.prevent_exit();
            }
        });
    Ok(())
}

#[cfg(test)]
mod tests;
