# Packaging and store submission

Build desktop packages on the target operating system. Start in `apps/desktop` and install the UI dependencies.

```sh
pnpm --dir ui --ignore-workspace install --frozen-lockfile
```

Run the built desktop executable with `--self-test` before releasing it. The test exercises an isolated clipboard, stale-write detection, the copy engine, and configuration persistence. On macOS it also binds and releases temporary F18 and F19 shortcuts. The commands below use the default Cargo output paths. Adjust them if you set `CARGO_TARGET_DIR`.

## macOS

Both channels build a universal bundle (Apple silicon and Intel) for macOS 13.5 or later. The bundle identifier is `md.copied.desktop`, and the main executable is `copied-md`. The Tauri config sets `macOSPrivateApi` to `false`.

Install the Apple silicon and Intel Rust targets before building.

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

### Mac App Store

The checked-in `src-tauri/tauri.appstore.conf.json` builds the `.app` with the App Sandbox entitlement and disables hardened runtime. Use the desktop app by itself for this channel. Launch at login uses `SMAppService` and is off by default.

1. Create a release-only config for the Apple Distribution signing identity and Mac App Store provisioning profile. Map the profile to `embedded.provisionprofile` under `bundle.macOS.files`.
2. Copy `entitlements.mas.plist` for the release and add `com.apple.application-identifier` (`<TEAMID>.md.copied.desktop`) and `com.apple.developer.team-identifier` to match the profile. Set `bundle.macOS.entitlements` to the release copy.
3. Set `APPLE_SIGNING_IDENTITY` or `bundle.macOS.signingIdentity` for app signing. Use a Mac Installer Distribution certificate for the `.pkg`.
4. Set `ITSAppUsesNonExemptEncryption` in the release Info.plist according to the app’s encryption use. [Tauri App Store distribution](https://v2.tauri.app/distribute/app-store/)

Apply the release config after the checked-in App Store config. Verify the signed app before packaging it for upload. Set `KEY_ID` and `ISSUER_ID` to your App Store Connect API credentials for the upload.

```sh
pnpm --dir ui tauri build --target universal-apple-darwin --bundles app \
  --config src-tauri/tauri.appstore.conf.json --config /path/to/appstore-release.conf.json
app="src-tauri/target/universal-apple-darwin/release/bundle/macos/copied.md.app"
codesign -d --entitlements - "$app"
codesign --verify --deep --strict --verbose=2 "$app"
xcrun productbuild --sign "3rd Party Mac Developer Installer: <Team Name> (<TEAMID>)" \
  --component "$app" /Applications copied.md.pkg
xcrun altool --upload-app --type macos --file copied.md.pkg \
  --apiKey "$KEY_ID" --apiIssuer "$ISSUER_ID"
```

In App Store Connect, describe the user-invoked clipboard reads and writes, the configurable global shortcut, and the opt-in launch at login.

### Developer ID (direct download)

The default config enables hardened runtime and uses the empty `src-tauri/entitlements.direct.plist`. Supply a Developer ID Application certificate and notarization credentials. Tauri accepts `APPLE_ID`, an app-specific `APPLE_PASSWORD`, and `APPLE_TEAM_ID`, or App Store Connect API credentials. [Tauri macOS signing and notarization](https://v2.tauri.app/distribute/sign/macos/)

```sh
export APPLE_SIGNING_IDENTITY="Developer ID Application: <Team Name> (<TEAMID>)"
pnpm --dir ui tauri build --target universal-apple-darwin --bundles app,dmg
app="src-tauri/target/universal-apple-darwin/release/bundle/macos/copied.md.app"
spctl --assess --type execute --verbose "$app"
xcrun stapler validate "$app"
```

#### Bundle the VS Code pasteboard helper

The default Tauri bundle contains the desktop executable. To support rich paste in VS Code, build the CLI for both Apple targets and use `lipo -create` to combine them under the name `copied-md-pasteboard`. Run the Cargo commands from the repository root.

```sh
cargo build --release --locked -p copied-cli --target aarch64-apple-darwin
cargo build --release --locked -p copied-cli --target x86_64-apple-darwin
lipo -create "target/aarch64-apple-darwin/release/copied-md" \
  "target/x86_64-apple-darwin/release/copied-md" \
  -output /absolute/path/to/copied-md-pasteboard
```

Add the helper to a Developer ID release override and pass it to the Tauri build with `--config`. This puts the helper at `Contents/Helpers/copied-md-pasteboard` before the app is signed and packaged. Use this override for direct downloads only. [Tauri macOS bundle configuration](https://v2.tauri.app/reference/config/#macconfig)

```json
{
  "bundle": {
    "macOS": {
      "files": {
        "Helpers/copied-md-pasteboard": "/absolute/path/to/copied-md-pasteboard"
      }
    }
  }
}
```

Sign the helper before running the Tauri build with the override. Tauri copies it into the bundle as a custom file.

```sh
codesign --force --sign "$APPLE_SIGNING_IDENTITY" --options runtime --timestamp \
  /absolute/path/to/copied-md-pasteboard
```

Verify the helper’s signature as well as the app’s. Install the app in Applications and test rich paste from local VS Code. You can also create a symlink named `copied-md` on your `PATH` to use the bundled helper as a CLI.

## Windows

Tauri builds NSIS (`.exe`) and MSI installers on Windows. For a signed release, set the Authenticode certificate thumbprint, `digestAlgorithm: "sha256"`, and `timestampUrl` under `bundle.windows`, or set `signCommand` for Trusted Signing. Pass the release config to the build with `--config`. [Tauri Windows code signing](https://v2.tauri.app/distribute/sign/windows/)

```powershell
pnpm --dir ui tauri build --bundles nsis --config "C:\path\to\windows-signing.conf.json"
Get-AuthenticodeSignature "src-tauri\target\release\bundle\nsis\*.exe"
```

For the Microsoft Store, submit a signed offline installer as an unpackaged Win32 app in Partner Center. Create `src-tauri/tauri.microsoftstore.conf.json` with the offline WebView2 install mode and a publisher name distinct from the product name (`copied.md`). [Tauri Microsoft Store distribution](https://v2.tauri.app/distribute/microsoft-store/)

```json
{
  "bundle": {
    "publisher": "<Publisher Name>",
    "windows": { "webviewInstallMode": { "type": "offlineInstaller" } }
  }
}
```

```powershell
pnpm --dir ui tauri build --bundles nsis --config "C:\path\to\windows-signing.conf.json" --config src-tauri/tauri.microsoftstore.conf.json
```

1. Enroll as a Microsoft developer and reserve the app name in Partner Center as an **EXE or MSI app**.
2. Host the signed installer at a stable HTTPS URL that serves a versioned file.
3. Supply the installer URL and silent install argument (`/S` for NSIS or `/quiet` for MSI).
4. Describe the clipboard access and global shortcut in the certification notes.

## Linux

The default Tauri bundle targets on Linux are AppImage, `.deb`, and `.rpm`. Build on the oldest distribution you plan to support so the packages use compatible WebKitGTK and glibc versions.

```sh
sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev \
  librsvg2-dev libxdo-dev libssl-dev patchelf build-essential
pnpm --dir ui tauri build --bundles appimage,deb,rpm
```

Sign the AppImage with `gpg --detach-sign` and publish the `.sig` beside it. Sign the `.deb` and `.rpm` through your repository tooling (`dpkg-sig` or an APT repository key, and `rpm --addsign`).

A Flatpak release needs a Flathub manifest. Include the runtime, permissions, and sources below.

- The GNOME runtime that ships WebKitGTK 4.1.
- `--socket=wayland`, `--socket=fallback-x11`, and `--share=ipc` for clipboard and shortcut access.
- `--talk-name=org.freedesktop.portal.Desktop` for the GlobalShortcuts portal.
- Offline Cargo and pnpm sources generated with `flatpak-cargo-generator` and `flatpak-node-generator`.

The Flatpak sandbox can limit access to Wayland data control. Check the app’s session capabilities in the packaged build.

### Session capabilities

Linux clipboard and shortcut support depends on the desktop session.

| Capability                    | Wayland                                                                                                | X11                                                              | No display  |
| ----------------------------- | ------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------- | ----------- |
| Background clipboard          | Requires compositor access to `ext_data_control_manager_v1` or `zwlr_data_control_manager_v1`.         | Uses the `CLIPBOARD` selection with XFixes change notifications. | Unavailable |
| Global shortcut               | When the GlobalShortcuts portal is available, the app directs you to your desktop’s keyboard settings. | Registers directly with X11.                                     | Unavailable |
| Active-app detection and pins | Uses the last-used format.                                                                             | Uses `_NET_ACTIVE_WINDOW` and `_NET_WM_PID` when available.      | Unavailable |
| Launch at login               | Uses XDG autostart when available.                                                                     | Uses XDG autostart when available.                               | Unavailable |

copied.md identifies Wayland from `WAYLAND_DISPLAY`, `WAYLAND_SOCKET`, or `XDG_SESSION_TYPE=wayland`. Otherwise, it uses X11 if `DISPLAY` is set.

## Browser extension

Run these commands from the repository root. WXT builds Manifest V3 for Chrome and Edge with `offscreen` clipboard writes and a Chromium minimum of 116. Firefox uses Manifest V2, requires Firefox 102 or later, and has the Gecko ID `copied-md@copied.md`. Chat-site permissions are optional in all three browsers.

```sh
pnpm zip            # Chrome
pnpm zip:edge       # Edge
pnpm zip:firefox    # Firefox, with the sources ZIP for AMO review
```

See [Chrome Web Store submission](chrome-web-store.md) and [Browser extension privacy](browser-extension-privacy.md).

## WebAssembly migration checklist

The browser and VS Code extensions use the TypeScript package `@0xdoublesharp/copied-md-converters`. Complete these checks before switching to `copied-wasm`.

- [ ] `cargo check --target wasm32-unknown-unknown -p copied-wasm` and a `wasm-bindgen` build produce a module under the size budget agreed for the extension.
- [ ] `convert_markdown_json` matches the TypeScript output for the golden corpus in oracle mode, and the documented improvements are accepted for the extension. [Destination formatting](tauri-rust-architecture.md#destination-formatting)
- [ ] Chrome and Edge MV3 load the module from the extension package with `wasm-unsafe-eval` in `content_security_policy.extension_pages` and no remote code.
- [ ] Firefox MV2 loads the module and passes AMO review with the Rust sources included in the sources ZIP.
- [ ] The VS Code extension loads the module in desktop VS Code and in the web extension host (`vscode.dev`).
- [ ] The offscreen clipboard path writes the same MIME types as before for every target.
- [ ] The TypeScript converter stays published until one release ships on WebAssembly without regressions.
