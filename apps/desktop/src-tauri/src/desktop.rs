use std::{collections::BTreeMap, path::PathBuf};

use copied_clipboard::Board;
use copied_config::{ConfigPatch, ConfigV2, ShortcutCopyMode, ShortcutSpec, save_config};
use copied_core::{
    ConversionOptions, CopyTarget, CopyTargetDefinition, DiagnosticSeverity, TARGETS,
    convert_markdown,
};
use copied_engine::{
    CopyEngine, EngineError, Notifier, OpId, PickerOpenRequest, ShortcutController,
};
use copied_platform::{
    ActiveApp, ActiveAppProvider, LoginItemBackend, LoginItemStatus, PlatformCapabilities,
    PlatformError, ShortcutBackend, ShortcutCapability,
};
use serde::Serialize;

use crate::{
    DesktopError,
    picker::{DesktopPicker, Panel, TauriWindowFactory, WindowFactory},
    platform::{DesktopBoard, DesktopShortcut, frontmost},
};

#[derive(Debug)]
pub(crate) enum Feedback {
    Copied(CopyTarget),
    CopiedMarkdown,
    Warning(String, String),
    Error(String),
}

#[derive(Default)]
pub(crate) struct DesktopNotifier(pub(crate) Vec<Feedback>);

impl Notifier for DesktopNotifier {
    fn mark_copied(&mut self, target: CopyTarget) {
        self.0.push(Feedback::Copied(target));
    }

    fn warn(&mut self, title: &str, detail: &str) {
        self.0.push(Feedback::Warning(title.into(), detail.into()));
    }

    fn error(&mut self, message: &str) {
        self.0.push(Feedback::Error(message.into()));
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopState {
    pub(crate) config: ConfigV2,
    pub(crate) targets: &'static [CopyTargetDefinition],
    pub(crate) active_app: Option<ActiveApp>,
    pub(crate) capabilities: PlatformCapabilities,
    pub(crate) login_item_status: LoginItemStatus,
    pub(crate) current_picker: Option<PickerOpenRequest>,
    pub(crate) panel: Option<Panel>,
    pub(crate) shortcut_registered: bool,
    pub(crate) shortcut_failure: Option<String>,
}

pub(crate) struct Desktop<F: WindowFactory = TauriWindowFactory> {
    pub(crate) engine: CopyEngine<DesktopBoard, DesktopPicker<F>, DesktopNotifier>,
    path: PathBuf,
    shortcut: ShortcutController<DesktopShortcut>,
    recording_previous: Option<ShortcutSpec>,
    shortcut_failure: Option<String>,
    capabilities: PlatformCapabilities,
    active_app: Option<Box<dyn ActiveAppProvider>>,
    focused_app: Option<ActiveApp>,
    login: Option<Box<dyn LoginItemBackend>>,
}

impl<F: WindowFactory> Desktop<F> {
    pub(crate) fn new(
        engine: CopyEngine<DesktopBoard, DesktopPicker<F>, DesktopNotifier>,
        path: PathBuf,
        shortcut: DesktopShortcut,
        capabilities: PlatformCapabilities,
        active_app: Option<Box<dyn ActiveAppProvider>>,
        login: Option<Box<dyn LoginItemBackend>>,
    ) -> Self {
        let mut desktop = Self {
            engine,
            path,
            shortcut: ShortcutController::new(shortcut),
            recording_previous: None,
            shortcut_failure: None,
            capabilities,
            active_app,
            focused_app: None,
            login,
        };
        if desktop.capabilities.global_shortcuts == ShortcutCapability::Direct
            && desktop.engine.config.shortcut_enabled
            && let Err(error) = desktop
                .shortcut
                .set_enabled(true, &mut desktop.engine.config)
        {
            desktop.shortcut_failure = Some(error.to_string());
        }
        desktop
    }

    pub(crate) fn state(&self) -> DesktopState {
        DesktopState {
            config: self.engine.config.clone(),
            targets: TARGETS,
            active_app: if self.engine.picker.panel.is_some() {
                self.focused_app.clone()
            } else {
                frontmost(&self.active_app)
            },
            capabilities: self.capabilities,
            login_item_status: self
                .login
                .as_ref()
                .map_or(LoginItemStatus::Unsupported, |login| login.status()),
            current_picker: self.engine.picker.request.clone(),
            panel: self.engine.picker.panel,
            shortcut_registered: self.shortcut.backend().current().is_some(),
            shortcut_failure: self.shortcut_failure.clone(),
        }
    }

    pub(crate) fn start(&mut self, mode: Option<ShortcutCopyMode>) -> Result<(), DesktopError> {
        if self.shortcut.is_recording() {
            return Ok(());
        }
        if self.engine.picker.panel.is_none() {
            self.focused_app = frontmost(&self.active_app);
        }
        let previous = self.engine.config.last_target;
        let mode = mode.unwrap_or_else(|| self.engine.mode());
        match self
            .engine
            .start_copy(mode, self.focused_app.as_ref().map(|app| app.id.as_str()))
        {
            Ok(copied_engine::CopyOutcome::PickerOpened(op_id))
                if self.engine.picker.failure.is_some() =>
            {
                let failure = self.engine.picker.failure.take();
                self.engine.handle_picker_dismiss(op_id)?;
                self.remember_copy(previous);
                if let Some(error) = failure {
                    return Err(error.into());
                }
            }
            Ok(_) => self.remember_copy(previous),
            Err(EngineError::Busy) => {}
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }

    pub(crate) fn choose(
        &mut self,
        op_id: OpId,
        target: Option<CopyTarget>,
    ) -> Result<(), DesktopError> {
        let previous = self.engine.config.last_target;
        match self.engine.handle_picker_choice(op_id, target) {
            Ok(_) => {
                self.cancel_recording_if_needed()?;
                self.remember_copy(previous);
            }
            Err(EngineError::StaleOpId) => {}
            Err(error) => {
                self.cancel_recording_if_needed()?;
                return Err(error.into());
            }
        }
        Ok(())
    }

    pub(crate) fn copy_html_as_markdown(&mut self) -> Result<(), DesktopError> {
        if self.shortcut.is_recording() {
            return Ok(());
        }
        match self.engine.convert_html_to_markdown() {
            Ok(_) => {
                // Replace the generic PlainText feedback emitted by CopyEngine with a clearer
                // Markdown toast so users immediately see the reverse conversion succeeded.
                if matches!(
                    self.engine.notifier.0.last(),
                    Some(Feedback::Copied(CopyTarget::PlainText))
                ) {
                    self.engine.notifier.0.pop();
                    self.engine.notifier.0.push(Feedback::CopiedMarkdown);
                }
                Ok(())
            }
            Err(EngineError::Busy) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    pub(crate) fn copy_sample(
        &mut self,
        markdown: &str,
        target: CopyTarget,
    ) -> Result<(), DesktopError> {
        let conversion = convert_markdown(markdown, target, &ConversionOptions::default());
        self.engine.board.write(&conversion.representations, None)?;
        self.engine.notifier.mark_copied(target);
        if let Some(warning) = conversion
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
        {
            self.engine
                .notifier
                .warn("Copied with a warning", &warning.message);
        }
        Ok(())
    }

    pub(crate) fn dismiss(&mut self, op_id: OpId) -> Result<(), DesktopError> {
        match self.engine.handle_picker_dismiss(op_id) {
            Ok(()) => self.cancel_recording_if_needed(),
            Err(EngineError::StaleOpId) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn remember_copy(&mut self, previous: Option<CopyTarget>) {
        if self.engine.config.last_target != previous
            && let Err(error) = save_config(&self.path, &self.engine.config)
        {
            self.engine.config.last_target = previous;
            self.engine
                .notifier
                .warn("Could not save the last-used format", &error.to_string());
        }
    }

    pub(crate) fn show_first_launch_welcome(&mut self) -> Result<(), DesktopError> {
        if !self.engine.config.has_launched {
            self.show_panel(Panel::Welcome)?;
            self.update_config(ConfigPatch {
                has_launched: Some(true),
                ..ConfigPatch::default()
            })?;
        }
        Ok(())
    }

    pub(crate) fn show_panel(&mut self, panel: Panel) -> Result<(), DesktopError> {
        if self.engine.picker.panel.is_none() {
            self.focused_app = frontmost(&self.active_app);
        }
        if let Some(request) = &self.engine.picker.request {
            self.dismiss(request.op_id)?;
        }
        if self.shortcut.is_recording() {
            self.cancel_recording()?;
        }
        if let Err(error) = self.engine.picker.show(panel) {
            self.engine.picker.hide();
            return Err(error.into());
        }
        Ok(())
    }

    pub(crate) fn close_panel(&mut self) -> Result<(), DesktopError> {
        if let Some(request) = &self.engine.picker.request {
            return self.dismiss(request.op_id);
        }
        self.engine.picker.hide();
        self.cancel_recording_if_needed()
    }

    pub(crate) fn update_config(&mut self, patch: ConfigPatch) -> Result<ConfigV2, DesktopError> {
        let update_binding = patch.shortcut.is_some() || patch.shortcut_enabled.is_some();
        let mut next = self.engine.config.clone();
        next.apply_patch(patch)?;
        self.save_with_binding(next, update_binding)?;
        Ok(self.engine.config.clone())
    }

    pub(crate) fn set_pin(
        &mut self,
        app_id: String,
        target: Option<CopyTarget>,
    ) -> Result<ConfigV2, DesktopError> {
        self.update_config(ConfigPatch {
            pinned_formats: Some(BTreeMap::from([(app_id, target)])),
            ..ConfigPatch::default()
        })
    }

    pub(crate) fn begin_recording(&mut self) -> Result<(), DesktopError> {
        self.require_direct_shortcut()?;
        if self.engine.picker.panel.is_none() {
            return Err(DesktopError::Unavailable);
        }
        let previous = self.shortcut.backend().current().cloned();
        self.shortcut.begin_recording()?;
        self.recording_previous = previous;
        Ok(())
    }

    pub(crate) fn commit_recording(
        &mut self,
        spec: ShortcutSpec,
    ) -> Result<ConfigV2, DesktopError> {
        let mut next = self.engine.config.clone();
        // Use config validation as well as the controller's validation, so the registered
        // shortcut and the persisted shortcut have the same supported key set.
        if let Err(error) = spec.validate() {
            self.cancel_recording_if_needed()?;
            return Err(error.into());
        }
        let previous = self.recording_previous.take();
        self.shortcut.finish_recording(spec, &mut next)?;
        self.save_after_binding(next, previous)?;
        Ok(self.engine.config.clone())
    }

    pub(crate) fn cancel_recording(&mut self) -> Result<(), DesktopError> {
        self.recording_previous = None;
        self.shortcut.cancel_recording()?;
        Ok(())
    }

    fn cancel_recording_if_needed(&mut self) -> Result<(), DesktopError> {
        if self.shortcut.is_recording() {
            self.cancel_recording()?;
        }
        Ok(())
    }

    pub(crate) fn reset_shortcut(&mut self) -> Result<ConfigV2, DesktopError> {
        self.require_direct_shortcut()?;
        let previous = self.shortcut.backend().current().cloned();
        let mut next = self.engine.config.clone();
        self.shortcut.reset_to_default(&mut next)?;
        self.save_after_binding(next, previous)?;
        Ok(self.engine.config.clone())
    }

    pub(crate) fn toggle_shortcut(&mut self, enabled: bool) -> Result<ConfigV2, DesktopError> {
        self.update_config(ConfigPatch {
            shortcut_enabled: Some(enabled),
            ..ConfigPatch::default()
        })
    }

    fn require_direct_shortcut(&self) -> Result<(), DesktopError> {
        if self.capabilities.global_shortcuts == ShortcutCapability::Direct {
            Ok(())
        } else {
            Err(PlatformError::Unsupported(
                "Keyboard shortcut registration is managed by your desktop session.".into(),
            )
            .into())
        }
    }

    fn save_with_binding(
        &mut self,
        mut next: ConfigV2,
        update_binding: bool,
    ) -> Result<(), DesktopError> {
        if update_binding {
            self.require_direct_shortcut()?;
            let previous = self.shortcut.backend().current().cloned();
            self.shortcut
                .set_enabled(next.shortcut_enabled, &mut next)?;
            self.save_after_binding(next, previous)
        } else {
            save_config(&self.path, &next)?;
            self.engine.config = next;
            Ok(())
        }
    }

    fn save_after_binding(
        &mut self,
        next: ConfigV2,
        previous: Option<ShortcutSpec>,
    ) -> Result<(), DesktopError> {
        if let Err(save) = save_config(&self.path, &next) {
            let mut restore = self.engine.config.clone();
            let enabled = previous.is_some();
            if let Some(spec) = previous {
                restore.shortcut = spec;
            }
            if let Err(error) = self.shortcut.set_enabled(enabled, &mut restore) {
                self.shortcut_failure = Some(error.to_string());
                return Err(DesktopError::ShortcutRestore { save, error });
            }
            return Err(save.into());
        }
        self.engine.config = next;
        self.shortcut_failure = None;
        Ok(())
    }

    pub(crate) fn toggle_login(&mut self, enabled: bool) -> Result<LoginItemStatus, DesktopError> {
        let backend = self
            .login
            .as_mut()
            .filter(|_| self.capabilities.launch_at_login)
            .ok_or_else(|| {
                PlatformError::Unsupported(
                    "Launch at Login is unavailable on this installation.".into(),
                )
            })?;
        Ok(backend.set_enabled(enabled)?)
    }
}
