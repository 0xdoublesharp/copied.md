# Packaging and store submission

This guide covers building, signing, and submitting copied.md 2 for each channel. Each section separates what the repository already configures from what the releaser supplies (certificates, profiles, store accounts, and any step that is not yet wired up).

Build the desktop app from `apps/desktop` on the target operating system after installing the UI dependencies:

```sh
pnpm --dir ui --ignore-workspace install --frozen-lockfile
```

Run `copied-md --self-test` from each built app before signing a release. It exercises an isolated clipboard, rejects a stale write, and round-trips the copy engine and configuration. On macOS it also binds and releases temporary F18 and F19 shortcuts.

## macOS

Both macOS channels ship a universal (`arm64` + `x86_64`) bundle with identifier `md.copied.desktop`, main binary `copied-md`, and a minimum of macOS 13.5. `macOSPrivateApi` is `false` in `tauri.conf.json`, so the app uses no private AppKit API in either channel.

Install both Apple Rust targets once:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

### Mac App Store

The repository configures:

- `src-tauri/tauri.appstore.conf.json`, which builds only the `.app`, uses `entitlements.mas.plist`, and turns off hardened runtime (App Store builds are sandboxed instead).
- `src-tauri/entitlements.mas.plist`, which grants only `com.apple.security.app-sandbox`. No JIT, no unsigned executable memory, no network client, and no file access entitlements.
- Launch at login through `SMAppService` (`copied-platform`), off until the user turns it on. App Store review requires this to be opt-in.

The App Store build contains no Node runtime, no bundled CLI, and no helper executables. Do not add the `copied-md` CLI helper to this build.

The releaser supplies:

1. An Apple Distribution certificate and a Mac App Store provisioning profile for `md.copied.desktop`. Add the profile through `bundle.macOS.files` as `embedded.provisionprofile` in a release-only config. Do not commit it.
2. A release-only copy of `entitlements.mas.plist` that adds `com.apple.application-identifier` (`<TEAMID>.md.copied.desktop`) and `com.apple.developer.team-identifier`, matching the profile.
3. The signing identity, through `APPLE_SIGNING_IDENTITY` or `bundle.macOS.signingIdentity` in a release-only config.
4. A Mac Installer Distribution certificate for the `.pkg`.

Build, package, and upload:

```sh
pnpm --dir ui tauri build --target universal-apple-darwin --bundles app \
  --config src-tauri/tauri.appstore.conf.json
xcrun productbuild --sign "3rd Party Mac Developer Installer: <Team Name> (<TEAMID>)" \
  --component "src-tauri/target/universal-apple-darwin/release/bundle/macos/copied.md.app" \
  /Applications copied.md.pkg
xcrun altool --upload-app --type macos --file copied.md.pkg \
  --apiKey <KEY_ID> --apiIssuer <ISSUER_ID>
```

Before submitting, check the sandbox and entitlements on the signed app:

```sh
codesign -d --entitlements - "copied.md.app"
codesign --verify --deep --strict --verbose=2 "copied.md.app"
```

In App Store Connect, explain that the app reads and writes the general pasteboard only when the user invokes a copy, and that it registers a global shortcut the user chooses. Tauri's guide covers the remaining App Store Connect steps: [Tauri App Store distribution](https://v2.tauri.app/distribute/app-store/).

### Developer ID (direct download)

The repository configures:

- The default `tauri.conf.json`, which turns on hardened runtime and uses `entitlements.direct.plist`.
- `src-tauri/entitlements.direct.plist`, an empty dictionary. The app needs no hardened-runtime exceptions.

The releaser supplies a Developer ID Application certificate and notarization credentials (`APPLE_ID`, `APPLE_PASSWORD` as an app-specific password, and `APPLE_TEAM_ID`, or an App Store Connect API key). Tauri signs and notarizes when these are set.

```sh
export APPLE_SIGNING_IDENTITY="Developer ID Application: <Team Name> (<TEAMID>)"
pnpm --dir ui tauri build --target universal-apple-darwin --bundles app,dmg
spctl --assess --type execute --verbose "src-tauri/target/universal-apple-darwin/release/bundle/macos/copied.md.app"
xcrun stapler validate "src-tauri/target/universal-apple-darwin/release/bundle/dmg/"*.dmg
```

The direct build should also carry the native `copied-md` CLI in `Contents/Helpers` so users can link it onto their `PATH`. This is not wired into `tauri.conf.json` yet. To add it for a release:

1. Build a universal CLI: `cargo build --release -p copied-cli --target aarch64-apple-darwin` and the same for `x86_64-apple-darwin`, then `lipo -create` the two binaries.
2. Copy it to `copied.md.app/Contents/Helpers/copied-md` before signing. The app's own executable is also named `copied-md`, but it lives in `Contents/MacOS`, so the names do not collide.
3. Sign the helper with the Developer ID identity, `--options runtime`, and a secure timestamp, then re-sign the app, notarize, and staple.
4. Never add the helper to the App Store build.

## Windows

The default Tauri bundle targets on Windows are NSIS (`.exe` installer) and MSI. The repository configures no certificate.

The releaser supplies an Authenticode code-signing certificate. An EV certificate or Azure Trusted Signing avoids most SmartScreen warnings on a new publisher. Set `bundle.windows.certificateThumbprint`, `digestAlgorithm: "sha256"`, and `timestampUrl` in a release-only config, or a `signCommand` for Trusted Signing.

```powershell
pnpm --dir ui tauri build --bundles nsis
Get-AuthenticodeSignature "src-tauri\target\release\bundle\nsis\*.exe"
```

For the Microsoft Store, submit the signed NSIS installer as an unpackaged Win32 app in Partner Center:

1. Reserve the app name in Partner Center.
2. Host the signed installer at a stable HTTPS URL that serves a versioned file.
3. Create a submission of type **EXE or MSI app**, give the installer URL, and set silent install arguments (`/S` for NSIS).
4. Describe the clipboard access and global shortcut in the certification notes.

An MSIX package is not configured.

## Linux

The default Tauri bundle targets on Linux are AppImage, `.deb`, and `.rpm`. Build each on the oldest distribution you support, since WebKitGTK and glibc versions are set at build time.

```sh
sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev \
  librsvg2-dev libxdo-dev libssl-dev patchelf build-essential
pnpm --dir ui tauri build --bundles appimage,deb,rpm
```

Sign the AppImage with `gpg --detach-sign` and publish the `.sig` beside it. Sign the `.deb` and `.rpm` through your repository tooling (`dpkg-sig` or an APT repository key, and `rpm --addsign`).

Flatpak is not configured in the repository. A Flathub manifest needs:

- the GNOME runtime that ships WebKitGTK 4.1;
- `--socket=wayland`, `--socket=fallback-x11`, and `--share=ipc` for clipboard and shortcut access;
- `--talk-name=org.freedesktop.portal.Desktop` for the GlobalShortcuts portal;
- offline Cargo and pnpm sources generated with `flatpak-cargo-generator` and `flatpak-node-generator`.

The Flatpak sandbox may block Wayland data-control access on some compositors. The capability report below then shows background clipboard as unavailable.

### Session capabilities

`copied-platform` reports what each Linux session supports. The desktop app reads the report at startup and disables the tray's launch-at-login and per-app pin items when they are unavailable. On Wayland, shortcut registration returns a message that points the user to the desktop's keyboard settings.

| Capability                  | Wayland                                                                                                                                 | X11                                                                                           | No display |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- | ---------- |
| Background clipboard        | Yes, when the compositor exposes `ext_data_control_manager_v1` or `zwlr_data_control_manager_v1` and grants access                      | Yes, through the `CLIPBOARD` selection with XFixes change notifications                       | No         |
| Global shortcut             | Detected through the GlobalShortcuts portal. The user assigns it in the desktop's keyboard settings; the app cannot register it itself. | Registered directly with X11                                                                  | No         |
| Active-app detection (pins) | No. Per-app pins never apply, and the remembered target is used.                                                                        | Yes, through `_NET_ACTIVE_WINDOW` and `_NET_WM_PID`, when the shortcut is registered directly | No         |
| Launch at login             | Yes, through an XDG autostart entry                                                                                                     | Yes, through an XDG autostart entry                                                           | No         |

The session counts as Wayland when `WAYLAND_DISPLAY` or `WAYLAND_SOCKET` is set or `XDG_SESSION_TYPE=wayland`. Otherwise it is X11 when `DISPLAY` is set. Compositors without a data-control protocol (GNOME's Mutter, for example) report no background clipboard.

## Browser extension

The extension builds with WXT from `apps/browser-extension`. Chrome and Edge get Manifest V3 (minimum Chrome 116, `offscreen` for clipboard writes, optional chat-site host permissions). Firefox gets WXT's default Manifest V2 build (minimum Firefox 102, gecko ID `copied-md@copied.md`, optional chat-site permissions).

```sh
pnpm zip            # Chrome
pnpm zip:edge       # Edge
pnpm zip:firefox    # Firefox, with the sources ZIP for AMO review
```

Store listings and permission explanations are in [chrome-web-store.md](chrome-web-store.md) and [browser-extension-privacy.md](browser-extension-privacy.md).

## WebAssembly cutover

The extensions still convert with the TypeScript package `@0xdoublesharp/copied-md-converters`. Switch them to `copied-wasm` only after every item below passes:

- [ ] `cargo check --target wasm32-unknown-unknown -p copied-wasm` and a `wasm-bindgen` build produce a module under the size budget agreed for the extension.
- [ ] `convert_markdown_json` matches the TypeScript output for the golden corpus in oracle mode, and the seven documented improvements are accepted for the extension.
- [ ] Chrome and Edge MV3 load the module from the extension package with `wasm-unsafe-eval` in `content_security_policy.extension_pages` and no remote code.
- [ ] Firefox MV2 loads the module and passes AMO review with the Rust sources included in the sources ZIP.
- [ ] The VS Code extension loads the module in desktop VS Code and in the web extension host (`vscode.dev`).
- [ ] The offscreen clipboard path writes the same MIME types as before for every target.
- [ ] The TypeScript converter stays published until one release ships on WebAssembly without regressions.
