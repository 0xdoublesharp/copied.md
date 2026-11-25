use copied_engine::{OpId, PickerHost, PickerOpenRequest};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub(crate) const WINDOW_LABEL: &str = "picker";
const WINDOW_WIDTH: f64 = 600.0;
const WINDOW_HEIGHT: f64 = 540.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Panel {
    Picker,
    Settings,
    AppPin,
    Shortcut,
    Welcome,
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
        let builder = WebviewWindowBuilder::new(
            &self.app,
            WINDOW_LABEL,
            WebviewUrl::App("index.html".into()),
        )
        .title("copied.md")
        .inner_size(WINDOW_WIDTH, WINDOW_HEIGHT)
        .always_on_top(true)
        .resizable(false)
        .center()
        .visible(false)
        .skip_taskbar(true);
        #[cfg(target_os = "macos")]
        let builder = builder
            .title("")
            .hidden_title(true)
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .visible_on_all_workspaces(true);
        builder.build()
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
        {
            if let Some(pid) =
                frontmost_pid().filter(|pid| Some(*pid) != i32::try_from(std::process::id()).ok())
            {
                self.previous_app = Some(pid);
            }
            macos_position::place(window)?;
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

#[cfg(target_os = "macos")]
mod macos_position {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSEvent, NSScreen, NSWindow};
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use tauri::WebviewWindow;

    use super::{WINDOW_HEIGHT, WINDOW_WIDTH};

    pub(super) fn place(window: &WebviewWindow) -> tauri::Result<()> {
        let mtm = MainThreadMarker::new().ok_or(tauri::Error::InvalidWindowHandle)?;
        let cursor = NSEvent::mouseLocation();
        let screen = NSScreen::screens(mtm)
            .iter()
            .find(|screen| contains_point(screen.frame(), cursor))
            .or_else(|| NSScreen::mainScreen(mtm));
        if let Some(screen) = screen {
            let ptr = window.ns_window()?.cast::<NSWindow>();
            // Tauri owns the NSWindow for the lifetime of this handle. AppKit access
            // is restricted to the main thread above.
            let native = unsafe { ptr.as_ref() }.ok_or(tauri::Error::InvalidWindowHandle)?;
            let content = CGRect::new(CGPoint::ZERO, CGSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));
            let size = native.frameRectForContentRect(content).size;
            native.setFrame_display(picker_frame(screen.visibleFrame(), size), false);
        }
        Ok(())
    }

    fn contains_point(frame: CGRect, point: CGPoint) -> bool {
        let max = frame.max();
        (frame.origin.x..max.x).contains(&point.x) && (frame.origin.y..max.y).contains(&point.y)
    }

    fn picker_frame(visible: CGRect, size: CGSize) -> CGRect {
        let width = size.width.min(visible.size.width);
        let height = size.height.min(visible.size.height);
        let x = visible.mid().x - width / 2.0;
        let space_below_top = visible.size.height - height;
        let offset = (visible.size.height * 0.28).min(space_below_top);
        // AppKit uses global points with a bottom-left origin for both rectangles.
        let y = visible.max().y - offset - height;
        CGRect::new(CGPoint::new(x, y), CGSize::new(width, height))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn picker_uses_the_cursor_screen_and_its_visible_frame() {
            let primary = CGRect::new(CGPoint::ZERO, CGSize::new(1440.0, 900.0));
            let secondary = CGRect::new(CGPoint::new(-1280.0, 900.0), CGSize::new(1280.0, 1024.0));
            let cursor = CGPoint::new(-1.0, 1920.0);
            assert!(!contains_point(primary, cursor));
            assert!(contains_point(secondary, cursor));

            let visible = CGRect::new(CGPoint::new(-1280.0, 900.0), CGSize::new(1280.0, 1000.0));
            let size = CGSize::new(WINDOW_WIDTH, WINDOW_HEIGHT);
            let frame = picker_frame(visible, size);
            assert_eq!(frame.size, size);
            assert_eq!(frame.mid().x, visible.mid().x);
            assert!((visible.max().y - frame.max().y - 280.0).abs() < 0.001);
        }

        #[test]
        fn picker_stays_inside_short_and_narrow_visible_frames() {
            let size = CGSize::new(WINDOW_WIDTH, WINDOW_HEIGHT);
            let visible = CGRect::new(CGPoint::new(1440.0, -450.0), CGSize::new(800.0, 600.0));
            let frame = picker_frame(visible, size);
            assert_eq!(frame.origin, CGPoint::new(1540.0, -450.0));
            assert_eq!(frame.size, size);

            let tiny = CGRect::new(visible.origin, CGSize::new(500.0, 350.0));
            assert_eq!(picker_frame(tiny, size), tiny);
        }
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
