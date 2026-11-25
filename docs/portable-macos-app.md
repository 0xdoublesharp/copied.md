# Swift macOS app build and packaging

These instructions build the Swift menu-bar app in `apps/macos`, which runs on macOS 13.5 or later on Apple silicon and Intel. The bundle includes the TypeScript converter, a universal Node runtime, and a universal pasteboard helper. For the Tauri desktop app, use the [desktop build and packaging guide](app-store-and-packaging.md#macos).

Install Xcode Command Line Tools and the repository dependencies, then run `pnpm build:macos-app`. It writes `copied.md.app` and `copied-md-macos-universal.zip` under `build/portable`. Set `COPIED_MD_APP_OUT` to use a different output directory. Set `COPIED_MD_NODE_CACHE` to override the runtime archive cache at `build/macos-runtimes`.

Run `pnpm build:macos` for the standalone pasteboard helper. Set `COPIED_MD_MACOS_OUT` to override its default output at `build/macos`.

## Signing and checks

For local development, the default build signs the app ad hoc. To package a public download, install a Developer ID Application signing identity and save notarization credentials under a keychain profile with `xcrun notarytool store-credentials`. Build into a new output directory with both settings supplied.

```sh
COPIED_MD_SIGN_IDENTITY='Developer ID Application: Your Name (TEAMID)' \
COPIED_MD_NOTARY_PROFILE='copied-md-notarization' \
pnpm build:macos-app
```

The build signs the app and its helpers with the hardened runtime, submits the app to Apple, staples the accepted ticket, and checks Gatekeeper before writing the ZIP. Publish that ZIP as the universal macOS release asset.

Run the bundled executable with `--self-test` to check converter output, isolated pasteboard writes, stale-write protection, and hotkey registration.

```sh
build/portable/copied.md.app/Contents/MacOS/copied-md --self-test
build/portable/copied.md.app/Contents/Helpers/copied-md-pasteboard self-test
```

For a release smoke test, use a macOS account that has never installed copied.md. Download the ZIP, unzip it in Finder, move the app to **Applications**, and open it. Install the copied.md VSIX in local VS Code. Select Markdown, run **Copy Markdown As…**, choose **Rich Text**, and paste into a rich-text editor. Check that VS Code uses the bundled helper without a custom path.

## Use the format grid

Copy Markdown in another app, then press `⌃⌘C`. Choose how the shortcut works in the menu bar under **Shortcut Behavior**.

- **Copy Last Used and Show Picker** copies immediately and opens the grid. Click another card to replace the copy, press Return to dismiss it, or switch to your paste destination. This is the default.
- **Show Picker First** waits for you to click a card or press Return to copy the highlighted format.
- **Copy Last Used Only** copies immediately without opening the grid.

Use **Copy Markdown As…** from the menu bar whenever you want to choose before copying. The gear menu saves your column count and visible formats. The app remembers a format after a successful clipboard write and uses the original Markdown if you replace an automatic copy.

To use a particular format in an app, focus that app and select **Choose Format for [app]…** from the clipboard menu. The shortcut uses the pinned format when that app is focused. In picker-first mode, it highlights that format. Choose **Use Last-Used Format** to remove the pin. Use **Copy Markdown As…** to pick a different format for a single copy.

Choose **Signal** for rich paste into Signal Desktop. Other apps can use the plain-text fallback. If you customized the keyboard shortcut, **Reset to Default Shortcut** switches it to `⌃⌘C`.

See the [macOS format grid in the app guide](https://www.justinsilver.com/libraries/copied-md/#macos-format-grid).
