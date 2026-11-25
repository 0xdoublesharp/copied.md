use copied_config::{ConfigV2, ShortcutSpec, default_shortcut};
use copied_platform::{PlatformError, ShortcutBackend};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ShortcutError {
    #[error("shortcut recording is already in progress")]
    AlreadyRecording,
    #[error("shortcut recording is not in progress")]
    NotRecording,
    #[error("press a letter or number with at least two modifiers, including Control or Command")]
    InvalidShortcut,
    #[error(transparent)]
    Platform(#[from] PlatformError),
    #[error("{attempt}; could not restore the previous shortcut: {restore}")]
    Rollback {
        attempt: Box<ShortcutError>,
        restore: PlatformError,
    },
}

#[derive(Debug)]
struct Recording {
    previous: Option<ShortcutSpec>,
}

/// Manages the registered binding and the recording transaction. Mutations take
/// the desktop's config so the shortcut and the copy engine share one config.
#[derive(Debug)]
pub struct ShortcutController<S: ShortcutBackend> {
    backend: S,
    recording: Option<Recording>,
}

impl<S: ShortcutBackend> ShortcutController<S> {
    pub fn new(backend: S) -> Self {
        Self {
            backend,
            recording: None,
        }
    }

    pub fn backend(&self) -> &S {
        &self.backend
    }

    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    pub fn begin_recording(&mut self) -> Result<(), ShortcutError> {
        self.ensure_idle()?;
        let previous = self.backend.current().cloned();
        if previous.is_some() {
            self.backend.unbind();
        }
        self.recording = Some(Recording { previous });
        Ok(())
    }

    pub fn finish_recording(
        &mut self,
        shortcut: ShortcutSpec,
        config: &mut ConfigV2,
    ) -> Result<(), ShortcutError> {
        let recording = self.recording.take().ok_or(ShortcutError::NotRecording)?;
        let result = if valid(&shortcut) {
            self.bind_and_remember(shortcut, config)
        } else {
            Err(ShortcutError::InvalidShortcut)
        };
        result.map_err(|attempt| match self.restore(recording) {
            Ok(()) => attempt,
            Err(restore) => ShortcutError::Rollback {
                attempt: Box::new(attempt),
                restore,
            },
        })
    }

    pub fn cancel_recording(&mut self) -> Result<(), ShortcutError> {
        let recording = self.recording.take().ok_or(ShortcutError::NotRecording)?;
        self.restore(recording)?;
        Ok(())
    }

    pub fn set_enabled(
        &mut self,
        enabled: bool,
        config: &mut ConfigV2,
    ) -> Result<(), ShortcutError> {
        self.ensure_idle()?;
        if enabled {
            self.backend.bind(&config.shortcut)?;
        } else {
            self.backend.unbind();
        }
        config.shortcut_enabled = enabled;
        Ok(())
    }

    /// Registers the default before committing it to the config.
    pub fn reset_to_default(&mut self, config: &mut ConfigV2) -> Result<(), ShortcutError> {
        self.ensure_idle()?;
        self.bind_and_remember(default_shortcut(), config)
    }

    fn ensure_idle(&self) -> Result<(), ShortcutError> {
        if self.is_recording() {
            Err(ShortcutError::AlreadyRecording)
        } else {
            Ok(())
        }
    }

    fn bind_and_remember(
        &mut self,
        shortcut: ShortcutSpec,
        config: &mut ConfigV2,
    ) -> Result<(), ShortcutError> {
        self.backend.bind(&shortcut)?;
        config.shortcut = shortcut;
        config.shortcut_enabled = true;
        Ok(())
    }

    fn restore(&mut self, recording: Recording) -> Result<(), PlatformError> {
        if let Some(previous) = recording.previous {
            self.backend.bind(&previous)?;
        }
        Ok(())
    }
}

fn valid(shortcut: &ShortcutSpec) -> bool {
    let modifiers = [shortcut.ctrl, shortcut.alt, shortcut.shift, shortcut.meta];
    (shortcut.ctrl || shortcut.meta)
        && modifiers.into_iter().filter(|&pressed| pressed).count() >= 2
        && matches!(
            shortcut.code.as_bytes(),
            [b'K', b'e', b'y', b'A'..=b'Z']
                | [b'D', b'i', b'g', b'i', b't', b'0'..=b'9']
                | [b'N', b'u', b'm', b'p', b'a', b'd', b'0'..=b'9']
        )
}

#[cfg(test)]
mod tests;
