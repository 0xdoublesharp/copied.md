# copied.md Desktop

The Tauri v2 desktop app starts quietly in the system tray (opening a one-time interactive Welcome guide on first launch). Copy Markdown from any app, then press the global shortcut (`⌃⌘C` on macOS, `Ctrl+Shift+Alt+C` on Windows/Linux) or choose **Copy Markdown As…** from the tray menu.

## Key capabilities

- **Spotlight-style floating picker:** opens on the display under your pointer with `1–9` / `⌘1–⌘9` quick keys, instant type-to-filter (`⌘K` or type anywhere), and hover-to-preview showing output length, platform limits, and calm diagnostic notes.
- **Auto-detect & per-app pins:** automatically chooses the right format for Slack, Telegram, Signal, Discord, Teams, Word, WhatsApp, and Messages, while letting you manage explicit per-app overrides in **Settings…**.
- **Last-Markdown memory:** re-triggering the shortcut before copying something else reuses your original Markdown source so you can paste the same snippet into multiple apps in different formats.
- **Copy Rich Text as Markdown:** converts copied HTML from VS Code, Google Docs, MathML, tables, and web pages back to GFM Markdown.
- **Zero-WebView `CopyOnly` & native HUD toast:** copy-only runs never allocate a WebView and confirm or report errors through a single non-blocking native HUD toast.

## Development & packaging

Run these commands from `apps/desktop`. The UI has its own pnpm lockfile, and `src-tauri` has its own Cargo lockfile.

```sh
pnpm --dir ui --ignore-workspace install --frozen-lockfile
pnpm --dir ui tauri dev
```

Build on each target operating system with `pnpm --dir ui tauri build`. The default targets include NSIS and MSI on Windows, and AppImage, deb, and rpm on Linux. Install both Apple Rust targets before building a universal macOS application bundle:

```sh
pnpm --dir ui tauri build --target universal-apple-darwin --bundles app
```

For the Mac App Store, add `--config src-tauri/tauri.appstore.conf.json` and supply your App Store signing identity and provisioning profile in the release configuration. For Developer ID distribution, the default macOS configuration enables hardened runtime and uses the direct-distribution entitlements. See [App Store and Packaging](../../docs/app-store-and-packaging.md).

Run `copied-md --self-test` on the built binary to exercise the isolated clipboard, reject a stale write, verify the copy engine and config round-trip, and (on macOS) bind and release temporary F18/F19 hotkeys.
