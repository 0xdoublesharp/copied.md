use super::*;

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Register(String),
    Unregister(String),
}

#[derive(Debug, Default)]
struct Backend {
    current: Option<ShortcutSpec>,
    events: Vec<Event>,
    reserved: Vec<String>,
}

impl ShortcutBackend for Backend {
    fn current(&self) -> Option<&ShortcutSpec> {
        self.current.as_ref()
    }

    fn bind(&mut self, shortcut: &ShortcutSpec) -> Result<(), PlatformError> {
        if self.current.as_ref() == Some(shortcut) {
            return Ok(());
        }
        self.events.push(Event::Register(shortcut.code.clone()));
        if self.reserved.contains(&shortcut.code) {
            return Err(PlatformError::ShortcutInUse(shortcut.code.clone()));
        }
        self.unbind();
        self.current = Some(shortcut.clone());
        Ok(())
    }

    fn unbind(&mut self) {
        if let Some(old) = self.current.take() {
            self.events.push(Event::Unregister(old.code));
        }
    }
}

fn shortcut(code: &str) -> ShortcutSpec {
    ShortcutSpec {
        code: code.into(),
        ..default_shortcut()
    }
}

fn enabled(code: &str, reserved: &[&str]) -> (ShortcutController<Backend>, ConfigV2) {
    let config = ConfigV2 {
        shortcut: shortcut(code),
        ..ConfigV2::default()
    };
    let backend = Backend {
        current: Some(config.shortcut.clone()),
        reserved: reserved.iter().map(|&code| code.into()).collect(),
        ..Backend::default()
    };
    (ShortcutController::new(backend), config)
}

#[test]
fn recording_pauses_binding_and_commits_a_valid_replacement() {
    let (mut controller, mut config) = enabled("KeyC", &[]);
    controller.begin_recording().unwrap();
    assert!(controller.is_recording());
    assert!(controller.backend().current().is_none());
    assert_eq!(
        controller.begin_recording(),
        Err(ShortcutError::AlreadyRecording)
    );
    assert_eq!(
        controller.set_enabled(false, &mut config),
        Err(ShortcutError::AlreadyRecording)
    );
    assert_eq!(
        controller.reset_to_default(&mut config),
        Err(ShortcutError::AlreadyRecording)
    );
    assert_eq!(config.shortcut, shortcut("KeyC"));

    controller
        .finish_recording(shortcut("Digit4"), &mut config)
        .unwrap();
    assert_eq!(config.shortcut, shortcut("Digit4"));
    assert_eq!(controller.backend().current(), Some(&config.shortcut));
    assert!(config.shortcut_enabled);
    assert!(!controller.is_recording());
    assert_eq!(
        controller.backend().events,
        [
            Event::Unregister("KeyC".into()),
            Event::Register("Digit4".into())
        ]
    );

    controller.begin_recording().unwrap();
    controller
        .finish_recording(shortcut("Numpad9"), &mut config)
        .unwrap();
    assert_eq!(config.shortcut, shortcut("Numpad9"));
    assert_eq!(controller.backend().current(), Some(&config.shortcut));
}

#[test]
fn failed_bind_and_invalid_recordings_restore_the_previous_binding() {
    let (mut controller, mut config) = enabled("KeyC", &["KeyX"]);
    let attempts = [
        (
            shortcut("KeyX"),
            PlatformError::ShortcutInUse("KeyX".into()).into(),
        ),
        (shortcut("ArrowLeft"), ShortcutError::InvalidShortcut),
        (
            ShortcutSpec {
                ctrl: false,
                meta: false,
                ..shortcut("KeyY")
            },
            ShortcutError::InvalidShortcut,
        ),
        (
            ShortcutSpec {
                ctrl: true,
                shift: false,
                alt: false,
                meta: false,
                ..shortcut("KeyY")
            },
            ShortcutError::InvalidShortcut,
        ),
    ];
    for (candidate, error) in attempts {
        controller.begin_recording().unwrap();
        assert_eq!(
            controller.finish_recording(candidate, &mut config),
            Err(error)
        );
        assert_eq!(config.shortcut, shortcut("KeyC"));
        assert!(config.shortcut_enabled);
        assert_eq!(controller.backend().current(), Some(&config.shortcut));
        assert!(!controller.is_recording());
    }
    assert_eq!(
        &controller.backend().events[..3],
        [
            Event::Unregister("KeyC".into()),
            Event::Register("KeyX".into()),
            Event::Register("KeyC".into())
        ]
    );
}

#[test]
fn cancellation_restores_only_a_binding_that_was_active() {
    let (mut controller, mut config) = enabled("KeyC", &[]);
    controller.begin_recording().unwrap();
    controller.cancel_recording().unwrap();
    assert_eq!(controller.backend().current(), Some(&config.shortcut));
    assert_eq!(
        controller.cancel_recording(),
        Err(ShortcutError::NotRecording)
    );
    assert_eq!(
        controller.finish_recording(shortcut("KeyX"), &mut config),
        Err(ShortcutError::NotRecording)
    );

    controller.set_enabled(false, &mut config).unwrap();
    controller.begin_recording().unwrap();
    controller.cancel_recording().unwrap();
    assert!(!config.shortcut_enabled);
    assert!(controller.backend().current().is_none());

    controller.set_enabled(true, &mut config).unwrap();
    assert!(config.shortcut_enabled);
    assert_eq!(controller.backend().current(), Some(&config.shortcut));
}

#[test]
fn resetting_registers_first_and_failed_reset_leaves_the_old_binding() {
    let (mut controller, mut config) = enabled("KeyY", &["KeyC"]);
    assert_eq!(
        controller.reset_to_default(&mut config),
        Err(PlatformError::ShortcutInUse("KeyC".into()).into())
    );
    assert_eq!(config.shortcut, shortcut("KeyY"));
    assert_eq!(controller.backend().current(), Some(&config.shortcut));
    assert_eq!(
        controller.backend().events,
        [Event::Register("KeyC".into())]
    );

    let (mut controller, mut config) = enabled("KeyY", &[]);
    controller.reset_to_default(&mut config).unwrap();
    assert_eq!(config.shortcut, default_shortcut());
    assert_eq!(
        controller.backend().events,
        [
            Event::Register("KeyC".into()),
            Event::Unregister("KeyY".into())
        ]
    );
}

#[test]
fn restore_failure_is_reported_and_the_committed_shortcut_can_be_reenabled() {
    let (mut controller, mut config) = enabled("KeyC", &["KeyC", "KeyX"]);
    controller.begin_recording().unwrap();
    assert_eq!(
        controller.finish_recording(shortcut("KeyX"), &mut config),
        Err(ShortcutError::Rollback {
            attempt: Box::new(PlatformError::ShortcutInUse("KeyX".into()).into()),
            restore: PlatformError::ShortcutInUse("KeyC".into()),
        })
    );
    assert_eq!(config.shortcut, shortcut("KeyC"));
    assert!(controller.backend().current().is_none());

    controller.set_enabled(false, &mut config).unwrap();
    assert_eq!(
        controller.set_enabled(true, &mut config),
        Err(PlatformError::ShortcutInUse("KeyC".into()).into())
    );
    assert!(!config.shortcut_enabled);
    controller.backend.reserved.clear();
    controller.set_enabled(true, &mut config).unwrap();
    assert_eq!(controller.backend().current(), Some(&config.shortcut));
}
