//! Fuzzes portable shortcut parsing and formatting.
//!
//! `ShortcutSpec::parse` must never panic, must only return valid shortcuts, and every
//! valid shortcut must round-trip through `format_portable` and `parse` (in any case).
#![no_main]

use arbitrary::Arbitrary;
use copied_config::ShortcutSpec;
use copied_fuzz::from_alphabet;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct Spec {
    key: u8,
    raw_code: Option<String>,
    ctrl: bool,
    alt: bool,
    shift: bool,
    meta: bool,
}

#[derive(Arbitrary, Debug)]
enum Input {
    Parse(String),
    /// Modifier-shaped tokens so the modifier loop is exercised.
    ParseTokens(Vec<u8>),
    RoundTrip(Spec),
}

const KEYS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

fn check_parse(input: &str) {
    if let Ok(spec) = ShortcutSpec::parse(input) {
        spec.validate().expect("parse only returns valid shortcuts");
        assert_eq!(
            ShortcutSpec::parse(&spec.format_portable()).ok(),
            Some(spec)
        );
    }
}

fn spec(spec: &Spec) -> ShortcutSpec {
    let code = spec.raw_code.clone().unwrap_or_else(|| {
        let key = char::from(KEYS[usize::from(spec.key) % KEYS.len()]);
        if key.is_ascii_digit() {
            format!("Digit{key}")
        } else {
            format!("Key{key}")
        }
    });
    ShortcutSpec {
        code,
        ctrl: spec.ctrl,
        alt: spec.alt,
        shift: spec.shift,
        meta: spec.meta,
    }
}

fuzz_target!(|input: Input| match input {
    Input::Parse(text) => check_parse(&text),
    Input::ParseTokens(bytes) => {
        let tokens: Vec<String> = bytes
            .chunks(2)
            .take(8)
            .map(|pair| {
                const WORDS: [&str; 12] = [
                    "Ctrl", "control", "Alt", "option", "Shift", "Meta", "cmd", "Command",
                    " ctrl ", "", "Hyper", "c",
                ];
                let word = WORDS[usize::from(pair[0]) % WORDS.len()];
                if pair.len() == 2 && pair[1] % 7 == 0 {
                    from_alphabet(&pair[1..], b"Aa1+ ")
                } else {
                    word.to_owned()
                }
            })
            .collect();
        check_parse(&tokens.join("+"));
    }
    Input::RoundTrip(raw) => {
        let spec = spec(&raw);
        let label = spec.format_portable();
        let _ = spec.format_macos();
        match spec.validate() {
            Ok(()) => {
                assert_eq!(
                    ShortcutSpec::parse(&label).ok().as_ref(),
                    Some(&spec),
                    "{label}"
                );
                assert_eq!(
                    ShortcutSpec::parse(&label.to_ascii_lowercase())
                        .ok()
                        .as_ref(),
                    Some(&spec),
                    "{label}"
                );
            }
            Err(_) => {
                // A valid parse must not invent an invalid spec's code or modifiers.
                if let Ok(parsed) = ShortcutSpec::parse(&label) {
                    assert_ne!(parsed, spec);
                }
            }
        }
    }
});
