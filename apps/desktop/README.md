# copied.md Desktop

Copy Markdown from another app, then press `⌃⌘C` on macOS or `Ctrl+Shift+Alt+C` on Windows or X11. Choose **Copy Markdown As…** from the tray menu to open the picker directly. On Wayland, check your [session capabilities](../../docs/app-store-and-packaging.md#session-capabilities) for clipboard and shortcut support.

## Choose how the shortcut works

- **Copy Last Used and Show Picker** copies immediately and opens the picker. Choose another format to replace the copy. This is the default.
- **Show Picker First** opens the picker and waits for you to choose.
- **Copy Last Used Only** copies immediately without opening the picker.

When the focused app has a visible format pinned, the shortcut uses that format. Otherwise, it tries to detect the app and use a visible matching format before falling back to your last-used format. Manage pins, auto-detection, and format visibility in **Settings…**. If the clipboard still holds the previous conversion, the next copy uses the original Markdown.

The picker shows the output, character count, and any available platform limits. Hover over a format to preview it, press `1`–`9` to choose a numbered format, or type to filter. Press `⌘K` on macOS or `Ctrl+K` on Windows or Linux to focus the filter. On macOS, the picker opens on the display under the pointer.

To convert copied HTML to GitHub Flavored Markdown, choose **Copy Rich Text as Markdown** from the tray menu.

## Build from source

Install the platform dependencies for Tauri, the Rust toolchain pinned in [rust-toolchain.toml](../../rust-toolchain.toml), and the Node and pnpm versions specified in [package.json](../../package.json). [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

Run from `apps/desktop`. The UI and `src-tauri` each have their own lockfile.

```sh
pnpm --dir ui --ignore-workspace install --frozen-lockfile
pnpm --dir ui tauri dev
```

Build on the target operating system with `pnpm --dir ui tauri build`. For a universal macOS bundle, install the Apple silicon and Intel Rust targets before building.

```sh
pnpm --dir ui tauri build --target universal-apple-darwin --bundles app
```

Run the built desktop executable with `--self-test` to check isolated clipboard operations, stale-write detection, the copy engine, and the configuration round trip. On macOS, the test also binds and releases temporary F18 and F19 shortcuts.

See the [packaging guide](../../docs/app-store-and-packaging.md) for signing, installers, and how to bundle the [VS Code pasteboard helper](../vscode/README.md).
