use copied_config::{ConfigPatch, ConfigV2, ShortcutSpec};
use copied_core::{ConversionOptions, CopyTarget, MarkdownConversion, convert_markdown};
use copied_engine::OpId;
use copied_platform::LoginItemStatus;
use tauri::AppHandle;

use crate::{
    DesktopError,
    desktop::{Desktop, DesktopState},
    with_desktop,
};

async fn on_main<T: Send + 'static>(
    app: AppHandle,
    action: impl FnOnce(&mut Desktop) -> Result<T, DesktopError> + Send + 'static,
) -> Result<T, String> {
    let (sender, mut receiver) = tauri::async_runtime::channel(1);
    let dispatcher = app.clone();
    dispatcher
        .run_on_main_thread(move || {
            let result = with_desktop(&app, action).map_err(|error| error.to_string());
            let _ = sender.try_send(result);
        })
        .map_err(|error| error.to_string())?;
    receiver
        .recv()
        .await
        .ok_or_else(|| "The desktop application is closing.".to_string())?
}

#[tauri::command]
pub(crate) async fn get_desktop_state(app: AppHandle) -> Result<DesktopState, String> {
    on_main(app, |desktop| Ok(desktop.state())).await
}

#[tauri::command]
pub(crate) async fn picker_choose(
    app: AppHandle,
    op_id: u64,
    target: Option<CopyTarget>,
) -> Result<(), String> {
    on_main(app, move |desktop| desktop.choose(OpId(op_id), target)).await
}

#[tauri::command]
pub(crate) async fn picker_dismiss(app: AppHandle, op_id: u64) -> Result<(), String> {
    on_main(app, move |desktop| desktop.dismiss(OpId(op_id))).await
}

#[tauri::command]
pub(crate) async fn close_panel(app: AppHandle) -> Result<(), String> {
    on_main(app, |desktop| {
        if desktop.engine.picker.request.is_none() {
            desktop.close_panel()?;
        }
        Ok(())
    })
    .await
}

#[tauri::command]
pub(crate) async fn update_config(app: AppHandle, patch: ConfigPatch) -> Result<ConfigV2, String> {
    on_main(app, move |desktop| desktop.update_config(patch)).await
}

#[tauri::command]
pub(crate) async fn set_app_pin(
    app: AppHandle,
    app_id: String,
    target: Option<CopyTarget>,
) -> Result<ConfigV2, String> {
    on_main(app, move |desktop| desktop.set_pin(app_id, target)).await
}

#[tauri::command]
pub(crate) async fn begin_shortcut_recording(app: AppHandle) -> Result<(), String> {
    on_main(app, Desktop::begin_recording).await
}

#[tauri::command]
pub(crate) async fn commit_shortcut_recording(
    app: AppHandle,
    spec: ShortcutSpec,
) -> Result<ConfigV2, String> {
    on_main(app, move |desktop| desktop.commit_recording(spec)).await
}

#[tauri::command]
pub(crate) async fn cancel_shortcut_recording(app: AppHandle) -> Result<(), String> {
    on_main(app, Desktop::cancel_recording).await
}

#[tauri::command]
pub(crate) async fn reset_shortcut(app: AppHandle) -> Result<ConfigV2, String> {
    on_main(app, Desktop::reset_shortcut).await
}

#[tauri::command]
pub(crate) async fn toggle_shortcut(app: AppHandle, enabled: bool) -> Result<ConfigV2, String> {
    on_main(app, move |desktop| desktop.toggle_shortcut(enabled)).await
}

#[tauri::command]
pub(crate) async fn toggle_login_item(
    app: AppHandle,
    enabled: bool,
) -> Result<LoginItemStatus, String> {
    on_main(app, move |desktop| desktop.toggle_login(enabled)).await
}

#[tauri::command]
pub(crate) fn preview_markdown(
    markdown: String,
    target: CopyTarget,
    source_url: Option<String>,
    decorative_unicode: bool,
) -> MarkdownConversion {
    convert_markdown(
        &markdown,
        target,
        &ConversionOptions {
            source_url,
            decorative_unicode,
        },
    )
}
