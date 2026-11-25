use crate::ConfigError;
use serde::{Deserialize, Serialize};

const CARBON_CMD: u32 = 256;
const CARBON_SHIFT: u32 = 512;
const CARBON_OPTION: u32 = 2048;
const CARBON_CTRL: u32 = 4096;

// ANSI letter and digit keys use the US physical layout. The function keys
// share the same Carbon virtual key codes across layouts.
const CARBON_KEYS: &[(u32, &str)] = &[
    (0x00, "KeyA"),
    (0x0B, "KeyB"),
    (0x08, "KeyC"),
    (0x02, "KeyD"),
    (0x0E, "KeyE"),
    (0x03, "KeyF"),
    (0x05, "KeyG"),
    (0x04, "KeyH"),
    (0x22, "KeyI"),
    (0x26, "KeyJ"),
    (0x28, "KeyK"),
    (0x25, "KeyL"),
    (0x2E, "KeyM"),
    (0x2D, "KeyN"),
    (0x1F, "KeyO"),
    (0x23, "KeyP"),
    (0x0C, "KeyQ"),
    (0x0F, "KeyR"),
    (0x01, "KeyS"),
    (0x11, "KeyT"),
    (0x20, "KeyU"),
    (0x09, "KeyV"),
    (0x0D, "KeyW"),
    (0x07, "KeyX"),
    (0x10, "KeyY"),
    (0x06, "KeyZ"),
    (0x1D, "Digit0"),
    (0x12, "Digit1"),
    (0x13, "Digit2"),
    (0x14, "Digit3"),
    (0x15, "Digit4"),
    (0x17, "Digit5"),
    (0x16, "Digit6"),
    (0x1A, "Digit7"),
    (0x1C, "Digit8"),
    (0x19, "Digit9"),
    (0x7A, "F1"),
    (0x78, "F2"),
    (0x63, "F3"),
    (0x76, "F4"),
    (0x60, "F5"),
    (0x61, "F6"),
    (0x62, "F7"),
    (0x64, "F8"),
    (0x65, "F9"),
    (0x6D, "F10"),
    (0x67, "F11"),
    (0x6F, "F12"),
    (0x69, "F13"),
    (0x6B, "F14"),
    (0x71, "F15"),
    (0x6A, "F16"),
    (0x40, "F17"),
    (0x4F, "F18"),
    (0x50, "F19"),
    (0x5A, "F20"),
];

/// Platform-neutral keyboard shortcut definition using W3C `KeyboardEvent.code`.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutSpec {
    pub code: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

impl ShortcutSpec {
    /// Requires an alphanumeric physical key and at least two modifiers, including Ctrl or Meta.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if !matches!(
            self.code.as_bytes(),
            [b'K', b'e', b'y', b'A'..=b'Z'] | [b'D', b'i', b'g', b'i', b't', b'0'..=b'9']
        ) {
            return Err(ConfigError::InvalidShortcut(format!(
                "expected a W3C letter or digit code, got {:?}",
                self.code
            )));
        }
        let count = [self.ctrl, self.alt, self.shift, self.meta]
            .into_iter()
            .filter(|&enabled| enabled)
            .count();
        if !(self.ctrl || self.meta) || count < 2 {
            return Err(ConfigError::InvalidShortcut(
                "at least two modifiers are required, including Ctrl or Meta".into(),
            ));
        }
        Ok(())
    }

    /// Uses the same modifier glyphs and order as the macOS shortcut label.
    pub fn format_macos(&self) -> String {
        let mut label = String::new();
        for (enabled, glyph) in [
            (self.ctrl, '⌃'),
            (self.alt, '⌥'),
            (self.shift, '⇧'),
            (self.meta, '⌘'),
        ] {
            if enabled {
                label.push(glyph);
            }
        }
        label.push_str(self.key_label());
        label
    }

    /// Returns a portable label such as `Ctrl+Shift+Alt+C`.
    pub fn format_portable(&self) -> String {
        let mut label = String::new();
        for (enabled, name) in [
            (self.ctrl, "Ctrl+"),
            (self.shift, "Shift+"),
            (self.alt, "Alt+"),
            (self.meta, "Meta+"),
        ] {
            if enabled {
                label.push_str(name);
            }
        }
        label.push_str(self.key_label());
        label
    }

    /// Parses a portable shortcut with a final ASCII letter or digit.
    pub fn parse(input: &str) -> Result<Self, ConfigError> {
        let mut tokens = input.split('+').map(str::trim);
        let key = tokens.next_back().unwrap_or_default();
        let code = match key.as_bytes() {
            [b'a'..=b'z' | b'A'..=b'Z'] => format!("Key{}", key.to_ascii_uppercase()),
            [b'0'..=b'9'] => format!("Digit{key}"),
            _ => return Err(ConfigError::InvalidShortcut(format!("invalid key {key:?}"))),
        };

        let mut shortcut = Self {
            code,
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
        };
        for token in tokens {
            let enabled = match token.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => &mut shortcut.ctrl,
                "alt" | "option" => &mut shortcut.alt,
                "shift" => &mut shortcut.shift,
                "meta" | "cmd" | "command" => &mut shortcut.meta,
                _ => {
                    return Err(ConfigError::InvalidShortcut(format!(
                        "unknown modifier {token:?}"
                    )));
                }
            };
            if *enabled {
                return Err(ConfigError::InvalidShortcut(format!(
                    "duplicate modifier {token:?}"
                )));
            }
            *enabled = true;
        }
        shortcut.validate()?;
        Ok(shortcut)
    }

    /// Decodes Carbon key and modifier values. Call `validate` to check recording eligibility.
    pub fn from_carbon(key_code: u32, modifiers: u32) -> Option<Self> {
        Some(Self {
            code: carbon_key_code_to_w3c(key_code)?.to_owned(),
            ctrl: modifiers & CARBON_CTRL != 0,
            alt: modifiers & CARBON_OPTION != 0,
            shift: modifiers & CARBON_SHIFT != 0,
            meta: modifiers & CARBON_CMD != 0,
        })
    }

    fn key_label(&self) -> &str {
        self.code
            .strip_prefix("Key")
            .or_else(|| self.code.strip_prefix("Digit"))
            .unwrap_or(&self.code)
    }
}

/// Translates an ANSI Carbon virtual key code to its W3C physical key code.
pub fn carbon_key_code_to_w3c(key_code: u32) -> Option<&'static str> {
    CARBON_KEYS
        .iter()
        .find_map(|&(carbon, code)| (carbon == key_code).then_some(code))
}

/// Translates a W3C physical key code to its ANSI Carbon virtual key code.
pub fn w3c_to_carbon_key_code(code: &str) -> Option<u32> {
    CARBON_KEYS
        .iter()
        .find_map(|&(carbon, w3c)| (w3c == code).then_some(carbon))
}

/// Returns the platform default shortcut (`Ctrl+Cmd+C` on macOS, `Ctrl+Shift+Alt+C` elsewhere).
pub fn default_shortcut() -> ShortcutSpec {
    let macos = cfg!(target_os = "macos");
    ShortcutSpec {
        code: String::from("KeyC"),
        ctrl: true,
        alt: !macos,
        shift: !macos,
        meta: macos,
    }
}
