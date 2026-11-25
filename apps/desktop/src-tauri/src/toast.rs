use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use tauri::AppHandle;

const TOAST_DURATION: Duration = Duration::from_millis(2800);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToastKind {
    Warning,
    Error,
}

/// Generation-guarded single-instance toast tracker so repeated errors update in place
/// instead of stacking modal dialogs or letting stale timers dismiss a newer toast.
#[derive(Clone, Default)]
pub(crate) struct ToastState {
    generation: Arc<AtomicU64>,
}

impl ToastState {
    pub(crate) fn next_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub(crate) fn should_hide(&self, expected: u64) -> bool {
        self.generation.load(Ordering::Relaxed) == expected
    }

    pub(crate) fn show(&self, app: &AppHandle, kind: ToastKind, title: &str, detail: &str) {
        let generation = self.next_generation();
        let state = self.clone();
        let title = title.to_owned();
        let detail = detail.to_owned();
        let show_app = app.clone();
        let _ = app.run_on_main_thread(move || {
            platform::show(&show_app, kind, &title, &detail);
        });
        let hide_app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            std::thread::sleep(TOAST_DURATION);
            if state.should_hide(generation) {
                let target = hide_app.clone();
                let _ = hide_app.run_on_main_thread(move || {
                    if state.should_hide(generation) {
                        platform::hide(&target);
                    }
                });
            }
        });
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::cell::RefCell;

    use objc2::{MainThreadMarker, MainThreadOnly, rc::Retained};
    use objc2_app_kit::{
        NSBackingStoreType, NSBox, NSBoxType, NSColor, NSFont, NSLineBreakMode, NSPanel, NSScreen,
        NSStatusWindowLevel, NSTextField, NSTitlePosition, NSVisualEffectBlendingMode,
        NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
        NSWindowCollectionBehavior, NSWindowStyleMask,
    };
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_foundation::NSString;
    use tauri::AppHandle;

    use super::ToastKind;

    const WIDTH: f64 = 332.0;
    const HEIGHT: f64 = 66.0;
    const MARGIN: f64 = 16.0;

    struct NativeToast {
        panel: Retained<NSPanel>,
        accent: Retained<NSBox>,
        title: Retained<NSTextField>,
        detail: Retained<NSTextField>,
    }

    thread_local! {
        static TOAST: RefCell<Option<NativeToast>> = const { RefCell::new(None) };
    }

    fn accent_color(kind: ToastKind) -> Retained<NSColor> {
        match kind {
            ToastKind::Error => NSColor::colorWithSRGBRed_green_blue_alpha(0.90, 0.28, 0.30, 1.0),
            ToastKind::Warning => NSColor::colorWithSRGBRed_green_blue_alpha(0.96, 0.65, 0.14, 1.0),
        }
    }

    fn target_frame(mtm: MainThreadMarker) -> CGRect {
        let visible = NSScreen::mainScreen(mtm)
            .map(|screen| screen.visibleFrame())
            .unwrap_or_else(|| CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(1280.0, 800.0)));
        let x = visible.origin.x + visible.size.width - WIDTH - MARGIN;
        let y = visible.origin.y + visible.size.height - HEIGHT - MARGIN;
        CGRect::new(CGPoint::new(x, y), CGSize::new(WIDTH, HEIGHT))
    }

    fn create_toast(mtm: MainThreadMarker) -> NativeToast {
        let frame = target_frame(mtm);
        let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
        let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
            NSPanel::alloc(mtm),
            frame,
            style,
            NSBackingStoreType::Buffered,
            false,
        );
        panel.setFloatingPanel(true);
        panel.setHidesOnDeactivate(false);
        panel.setOpaque(false);
        panel.setHasShadow(true);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setLevel(NSStatusWindowLevel);
        panel.setIgnoresMouseEvents(true);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );

        let bounds = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(WIDTH, HEIGHT));
        let effect = NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), bounds);
        effect.setMaterial(NSVisualEffectMaterial::HUDWindow);
        effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        effect.setState(NSVisualEffectState::Active);
        effect.setWantsLayer(true);
        if let Some(layer) = effect.layer() {
            layer.setCornerRadius(12.0);
            layer.setMasksToBounds(true);
        }

        let accent = NSBox::initWithFrame(
            NSBox::alloc(mtm),
            CGRect::new(CGPoint::new(14.0, 15.0), CGSize::new(4.0, HEIGHT - 30.0)),
        );
        accent.setBoxType(NSBoxType::Custom);
        accent.setTitlePosition(NSTitlePosition::NoTitle);
        accent.setBorderWidth(0.0);
        accent.setCornerRadius(2.0);
        accent.setFillColor(&accent_color(ToastKind::Error));

        let title = NSTextField::labelWithString(&NSString::from_str(""), mtm);
        title.setFrame(CGRect::new(
            CGPoint::new(28.0, HEIGHT - 28.0),
            CGSize::new(WIDTH - 42.0, 18.0),
        ));
        title.setFont(Some(&NSFont::boldSystemFontOfSize(13.0)));
        title.setTextColor(Some(&NSColor::labelColor()));
        title.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);

        let detail = NSTextField::labelWithString(&NSString::from_str(""), mtm);
        detail.setFrame(CGRect::new(
            CGPoint::new(28.0, 12.0),
            CGSize::new(WIDTH - 42.0, 22.0),
        ));
        detail.setFont(Some(&NSFont::systemFontOfSize(12.0)));
        detail.setTextColor(Some(&NSColor::secondaryLabelColor()));
        detail.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);

        effect.addSubview(&accent);
        effect.addSubview(&title);
        effect.addSubview(&detail);
        panel.setContentView(Some(&effect));

        NativeToast {
            panel,
            accent,
            title,
            detail,
        }
    }

    pub(super) fn show(_app: &AppHandle, kind: ToastKind, title: &str, detail: &str) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        TOAST.with(|slot| {
            let mut slot = slot.borrow_mut();
            let toast = slot.get_or_insert_with(|| create_toast(mtm));
            toast.panel.setFrame_display(target_frame(mtm), false);
            toast.accent.setFillColor(&accent_color(kind));
            toast.title.setStringValue(&NSString::from_str(title));
            toast.detail.setStringValue(&NSString::from_str(detail));
            toast.panel.orderFrontRegardless();
        });
    }

    pub(super) fn hide(_app: &AppHandle) {
        TOAST.with(|slot| {
            if let Some(toast) = slot.borrow().as_ref() {
                toast.panel.orderOut(None);
            }
        });
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use tauri::AppHandle;

    use super::ToastKind;

    pub(super) fn show(app: &AppHandle, _kind: ToastKind, title: &str, detail: &str) {
        if let Some(tray) = app.tray_by_id(crate::tray::TRAY_ID) {
            let _ = tray.set_tooltip(Some(format!("{title}: {detail}")));
        }
    }

    pub(super) fn hide(app: &AppHandle) {
        if let Some(tray) = app.tray_by_id(crate::tray::TRAY_ID) {
            let _ = tray.set_tooltip(Some("copied.md"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_toasts_supersede_earlier_hide_timers() {
        let state = ToastState::default();
        let first = state.next_generation();
        assert!(state.should_hide(first));

        let second = state.next_generation();
        assert_ne!(first, second);
        assert!(!state.should_hide(first));
        assert!(state.should_hide(second));
    }
}
