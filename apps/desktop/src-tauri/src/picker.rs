use copied_engine::{OpId, PickerHost, PickerOpenRequest};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub(crate) const WINDOW_LABEL: &str = "picker";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Panel {
    Picker,
    Settings,
    AppPin,
    Shortcut,
}

pub(crate) trait WindowFactory: Send {
    type Window: Send;

    fn create(&mut self) -> tauri::Result<Self::Window>;
    fn show(
        &mut self,
        window: &Self::Window,
        panel: Panel,
        request: Option<&PickerOpenRequest>,
    ) -> tauri::Result<()>;
    fn hide(&mut self, window: &Self::Window) -> tauri::Result<()>;
}

pub(crate) struct DesktopPicker<F: WindowFactory> {
    factory: F,
    window: Option<F::Window>,
    pub(crate) request: Option<PickerOpenRequest>,
    pub(crate) panel: Option<Panel>,
    pub(crate) failure: Option<tauri::Error>,
}

impl<F: WindowFactory> DesktopPicker<F> {
    pub(crate) fn new(factory: F) -> Self {
        Self {
            factory,
            window: None,
            request: None,
            panel: None,
            failure: None,
        }
    }

    pub(crate) fn show(&mut self, panel: Panel) -> tauri::Result<()> {
        let window = match &self.window {
            Some(window) => window,
            None => self.window.insert(self.factory.create()?),
        };
        self.factory.show(window, panel, self.request.as_ref())?;
        self.panel = Some(panel);
        Ok(())
    }

    pub(crate) fn hide(&mut self) {
        self.panel = None;
        if let Some(window) = &self.window
            && let Err(error) = self.factory.hide(window)
        {
            self.failure = Some(error);
        }
    }
}

impl<F: WindowFactory> PickerHost for DesktopPicker<F> {
    fn open(&mut self, request: PickerOpenRequest) {
        self.request = Some(request);
        if let Err(error) = self.show(Panel::Picker) {
            self.failure = Some(error);
        }
    }

    fn close(&mut self, op_id: OpId) {
        if self
            .request
            .as_ref()
            .is_some_and(|request| request.op_id == op_id)
        {
            self.request = None;
            self.hide();
        }
    }
}

pub(crate) struct TauriWindowFactory {
    app: AppHandle,
    #[cfg(target_os = "macos")]
    previous_app: Option<i32>,
}

impl TauriWindowFactory {
    pub(crate) fn new(app: AppHandle) -> Self {
        Self {
            app,
            #[cfg(target_os = "macos")]
            previous_app: None,
        }
    }
}

#[cfg(target_os = "macos")]
fn frontmost_pid() -> Option<i32> {
    objc2_app_kit::NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .map(|app| app.processIdentifier())
}

impl WindowFactory for TauriWindowFactory {
    type Window = WebviewWindow;

    fn create(&mut self) -> tauri::Result<Self::Window> {
        WebviewWindowBuilder::new(
            &self.app,
            WINDOW_LABEL,
            WebviewUrl::App("index.html".into()),
        )
        .title("copied.md")
        .inner_size(560.0, 490.0)
        .min_inner_size(400.0, 320.0)
        .center()
        .visible(false)
        .skip_taskbar(true)
        .build()
    }

    fn show(
        &mut self,
        window: &Self::Window,
        panel: Panel,
        request: Option<&PickerOpenRequest>,
    ) -> tauri::Result<()> {
        if let Some(request) = request {
            window.emit_to(WINDOW_LABEL, "picker:open", request)?;
        } else {
            window.emit_to(WINDOW_LABEL, "desktop:panel", panel)?;
        }
        #[cfg(target_os = "macos")]
        if let Some(pid) =
            frontmost_pid().filter(|pid| Some(*pid) != i32::try_from(std::process::id()).ok())
        {
            self.previous_app = Some(pid);
        }
        window.show()?;
        window.set_focus()
    }

    fn hide(&mut self, window: &Self::Window) -> tauri::Result<()> {
        #[cfg(target_os = "macos")]
        let restore = self
            .previous_app
            .take()
            .filter(|_| frontmost_pid() == i32::try_from(std::process::id()).ok());
        window.hide()?;
        #[cfg(target_os = "macos")]
        if let Some(app) = restore
            .and_then(objc2_app_kit::NSRunningApplication::runningApplicationWithProcessIdentifier)
        {
            app.activateWithOptions(objc2_app_kit::NSApplicationActivationOptions::empty());
        }
        Ok(())
    }
}

pub(crate) fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if window.label() != WINDOW_LABEL {
        return;
    }
    let close = match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            true
        }
        tauri::WindowEvent::Focused(false) => false,
        _ => return,
    };
    let app = window.app_handle().clone();
    // Native show/hide can emit focus events while the desktop state is locked.
    tauri::async_runtime::spawn(async move {
        let dispatcher = app.clone();
        let _ = dispatcher.run_on_main_thread(move || {
            if close
                || app.get_webview_window(WINDOW_LABEL).is_some_and(|window| {
                    window.is_visible().unwrap_or(false) && !window.is_focused().unwrap_or(true)
                })
            {
                let result = crate::with_desktop(&app, |desktop| desktop.close_panel());
                crate::tray::present_operation_error(&app, result);
            }
        });
    });
}
