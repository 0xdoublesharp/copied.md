# copied.md for macOS

## Product behavior

A single menu-bar app provides the conversion flow. Open it once and the default global shortcut is available immediately. The first-launch popover shows the key and offers Copy from Clipboard. Reopening the app shows the same controls.

The app registers Control–Option–Command–M through the system hotkey API. The menu can rebind, disable, or reset it and always offers the picker. A failed rebind keeps the previous working key. Quitting releases the key. Launch at Login is opt-in.

The system clipboard is read when the user invokes conversion. The app snapshots it before showing the native picker and checks its change count before writing the result. It writes plain text and, where available, HTML. Canceling leaves the clipboard ready for the previous paste. Conversion is local and uses the shared TypeScript converter.

## Packaging decision

Gemini recommended trying JavaScriptCore for a smaller bundle. The probe reproduced a fidelity regression on a standard HTTPS link because the bare context lacks the URL API. The universal bundle includes a verified Node runtime and the compiled converter, so desktop output uses the same runtime behavior as the CLI. Both runtime slices require macOS 13.5.

A developer build is locally signed. Developer ID signing and notarization are the release gates for a downloaded build on another Mac.

## Verification

Run the packaged app’s `--self-test` as arm64 and x86_64 in a clean environment. It exercises the packaged converter, both pasteboard flavors, stale-write protection, and system hotkey registration and release. The interactive acceptance check presses the key from another app and verifies that the picker appears without Shortcuts setup.
