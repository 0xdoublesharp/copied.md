#![cfg(target_os = "linux")]

use copied_config::ShortcutSpec;
use copied_platform::{PlatformError, ShortcutBackend, linux::X11HotKey};

#[test]
#[ignore = "run only against a disposable X11 session"]
fn exclusive_grabs_survive_a_failed_rebind_and_release_on_drop() {
    let first = ShortcutSpec::parse("Ctrl+Shift+A").unwrap();
    let second = ShortcutSpec::parse("Ctrl+Shift+B").unwrap();
    let mut primary = X11HotKey::new(|| {}).unwrap();
    let mut other = X11HotKey::new(|| {}).unwrap();
    primary.bind(&first).unwrap();
    other.bind(&second).unwrap();
    assert!(matches!(
        primary.bind(&second),
        Err(PlatformError::ShortcutInUse(_))
    ));
    assert_eq!(primary.current(), Some(&first));
    drop(other);
    primary.bind(&second).unwrap();
    assert_eq!(primary.current(), Some(&second));
    primary.unbind();
    assert_eq!(primary.current(), None);
}
