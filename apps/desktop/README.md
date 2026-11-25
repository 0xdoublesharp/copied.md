# copied.md Desktop

The Tauri app starts in the system tray. Copy Markdown from any app, then choose **Copy Markdown As…** from the tray menu. Use **Shortcut Behavior** to configure the global keyboard shortcut.

Run these commands from `apps/desktop`. The UI has its own pnpm lockfile, and `src-tauri` has its own Cargo lockfile.

```sh
pnpm --dir ui --ignore-workspace install --frozen-lockfile
pnpm --dir ui tauri dev
```

Build on each target operating system with `pnpm --dir ui tauri build`. The default targets include NSIS and MSI on Windows, and AppImage, deb, and rpm on Linux. Install both Apple Rust targets before building a universal macOS application bundle.

```sh
pnpm --dir ui tauri build --target universal-apple-darwin --bundles app
```

For the Mac App Store, add `--config src-tauri/tauri.appstore.conf.json` and supply your App Store signing identity and provisioning profile in the release configuration. For Developer ID distribution, the default macOS configuration enables hardened runtime and uses the direct-distribution entitlements. [Tauri macOS signing and App Store packaging](https://v2.tauri.app/distribute/app-store/)

Run the built `copied-md --self-test` binary to exercise the isolated clipboard, reject a stale write, and check the copy engine and configuration round-trip. On macOS the command also binds and releases temporary F18 and F19 shortcuts.
