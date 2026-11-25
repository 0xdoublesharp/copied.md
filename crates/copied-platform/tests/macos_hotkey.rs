#[cfg(target_os = "macos")]
fn main() {
    use copied_config::ShortcutSpec;
    use copied_platform::{PlatformError, ShortcutBackend, macos::MacHotKey};
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

    let main = MainThreadMarker::new().expect("native hotkey test must run on the main thread");
    let app = NSApplication::sharedApplication(main);
    app.setActivationPolicy(NSApplicationActivationPolicy::Prohibited);
    let chord = |code: &str| ShortcutSpec {
        code: code.into(),
        ctrl: true,
        alt: true,
        shift: true,
        meta: true,
    };
    let first = chord("F18");
    let next = chord("F19");
    let mut primary = MacHotKey::new(|| {}).unwrap();
    let mut other = MacHotKey::new(|| {}).unwrap();
    primary.bind(&first).unwrap();
    assert_eq!(primary.current(), Some(&first));
    other.bind(&next).unwrap();
    assert!(matches!(
        primary.bind(&next),
        Err(PlatformError::ShortcutInUse(_))
    ));
    assert_eq!(primary.current(), Some(&first));
    other.unbind();
    primary.bind(&next).unwrap();
    assert_eq!(primary.current(), Some(&next));
    other.bind(&first).unwrap();
    primary.unbind();
    assert_eq!(primary.current(), None);
    other.bind(&next).unwrap();
    drop(other);
    primary.bind(&next).unwrap();
    println!("native F18/F19 hotkey bind, conflict, replacement, unbind, and drop passed");
}

#[cfg(not(target_os = "macos"))]
fn main() {}
