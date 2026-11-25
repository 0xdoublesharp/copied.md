# Tauri and Rust architecture

The Tauri v2 desktop app and native CLI use the same Rust conversion and clipboard crates. The browser extension, VS Code extension, and Node CLI use the TypeScript converter. The workspace also exposes the Rust converter through `copied-wasm`. See the [WebAssembly migration checklist](app-store-and-packaging.md#webassembly-migration-checklist).

## Crates

| Crate              | Role                                                                                                                                                                                                                                  | Platform code                                                                                     |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `copied-codec`     | Reads and writes Chromium Pickle custom data (Signal) and Windows `CF_HTML`, including the `-1` no-context envelope. Escapes text for ASCII-safe RTF. `no_std + alloc`, `forbid(unsafe_code)`.                                        | None                                                                                              |
| `copied-core`      | Parses GitHub Flavored Markdown into 15 destination targets (`schemaVersion: 1` payload with metrics and diagnostics), owns the URL policy (`safe_url`), and provides reverse `html_to_markdown` (VS Code, Docs, MathML, GFM tables). | None                                                                                              |
| `copied-config`    | Versioned desktop settings (`ConfigV2`), format visibility, built-in destination app auto-detection, per-app pins, shortcut parsing, legacy macOS preference migration, and atomic persistence.                                       | None                                                                                              |
| `copied-engine`    | Manages copy sessions and shortcut rebinding. Uses increasing picker IDs to reject stale replies, keeps the original Markdown for repeat copies, and handles HTML → Markdown clipboard conversion.                                    | None                                                                                              |
| `copied-clipboard` | Revision-checked clipboard text/HTML reads and multi-flavor writes. Rejects writes when revision checks detect a newer copy.                                                                                                          | macOS `NSPasteboard`, Windows `OpenClipboard`, Linux Wayland data-control and X11 selections      |
| `copied-platform`  | Global shortcuts, active-app detection, launch at login, and a capability report per session.                                                                                                                                         | macOS `SMAppService`, Windows registry and hotkeys, Linux X11 hotkeys, XDG autostart, and portals |
| `copied-cli`       | Native `copied-md` CLI, cross-platform clipboard commands, stdio MCP server, and `copied-md-pasteboard` alias.                                                                                                                        | Uses `copied-clipboard`                                                                           |
| `copied-wasm`      | Exposes `copied-core` through `wasm-bindgen`.                                                                                                                                                                                         | `wasm32-unknown-unknown`                                                                          |

The desktop app lives in `apps/desktop`. `src-tauri` is a separate Cargo package with its own lockfile and is excluded from the root workspace, as is `fuzz/`. The Vue 3 picker, settings, and onboarding UI in `apps/desktop/ui` has its own pnpm lockfile.

The platform-independent crates (`copied-codec`, `copied-core`, `copied-config`, and `copied-engine`) run on each CI operating system. CI enforces line coverage thresholds on Ubuntu.

## Desktop windows and notifications

The tray app creates its WebView on demand for the picker, Settings, or the first-run welcome panel. A `CopyOnly` operation never opens the picker. `apps/desktop/src-tauri/src/picker.rs` reuses the window between sessions. On macOS it opens on the display under the pointer and is available on every Space.

Copy feedback uses a native, non-activating `NSPanel` on macOS and the tray tooltip on other platforms. `apps/desktop/src-tauri/src/toast.rs` uses a generation counter so an older dismissal timer cannot clear newer feedback.

## Destination formatting

`copied-core` reproduces the frozen `ts-oracle-v1` TypeScript engine byte for byte through `copied_core::oracle::convert_markdown_ts_v1`. The public `convert_markdown` uses the destination-specific behavior below. [Documented differences from the TypeScript baseline](../crates/copied-core/tests/parity/divergences.toml)

| Improvement                                                                     | Targets                               |
| ------------------------------------------------------------------------------- | ------------------------------------- |
| Combines nested instances of the same emphasis style                            | Slack API, WhatsApp, Telegram, Signal |
| Telegram plain text emits native Telegram Markdown (splitting bold around code) | Telegram                              |
| Omits RTF so Telegram for macOS can use its Markdown paste format               | Telegram                              |
| Native Telegram `<tg-spoiler>` and `\|\|spoiler\|\|` with fail-closed fallback  | Telegram                              |
| Discord `\|\|spoilers\|\|` and Reddit `>!spoilers!<` with fail-closed fallback  | Discord, Reddit                       |
| Aligned columns for tables shown in monospace                                   | Slack paste, Discord, WhatsApp        |
| Slack API table cells keep literal `\|` inside fenced code blocks               | Slack API                             |
| Dark-mode-safe code/header/quote contrast and preserved GFM header alignment    | Teams, Word                           |
| Visible `☑` / `☐` checkbox glyphs for task lists                                | Rich Text, Slack paste, Teams, Word   |
| Raw HTML blocks kept as separate Markdown blocks                                | Discord, Reddit                       |
| Four-space-indented code blocks for Old and New Reddit                          | Reddit                                |

Every target uses `safe_url` to keep fragments and allowed URL schemes (`http:`, `https:`, and `mailto:`). Relative links resolve against `sourceUrl` when supplied. Unresolved relative links produce a `relative-url` diagnostic, and rejected URL schemes produce `unsupported-url`.

## Verification

| Layer                       | What it checks                                                                                                                                                              | Where                                                                                                                                                                                         |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Golden parity               | Compares oracle mode with 960 frozen TypeScript outputs and checks current output against the reviewed divergences and golden file.                                         | [Parity test](../crates/copied-core/tests/parity.rs), [oracle fixtures](../crates/copied-core/tests/golden/corpus.json), [current fixtures](../crates/copied-core/tests/parity/improved.json) |
| Differential corpus         | Compares oracle mode with a generated TypeScript corpus supplied through `COPIED_ORACLE_JSON`. The test is opt-in.                                                          | `crates/copied-core/tests/differential.rs`                                                                                                                                                    |
| Coverage                    | Requires 100% line coverage for `copied-codec`, `copied-config`, and `copied-engine`, and at least 98% for `copied-core`.                                                   | CI `rust` job on Ubuntu                                                                                                                                                                       |
| Property tests (`proptest`) | Payload, metric, and determinism invariants across all targets; safe HTML; balanced RTF; fail-closed spoilers; codec round trips; config invariants; engine state machine.  | `crates/*/tests`, `crates/copied-config/src/tests.rs`                                                                                                                                         |
| `cargo-fuzz`                | Seven libFuzzer targets: `convert_all_targets`, `safe_url`, `pickle_decode`, `cf_html_parse`, `rtf_roundtrip`, `config_migrate`, and `shortcut_parse`.                      | `fuzz/`                                                                                                                                                                                       |
| TLA+ (TLC)                  | Six configurations: four passing (`Clipboard`, `WinBoard`, `CopyAndPick`, `Engine`) and two expected counterexamples (`LegacyClipboard`, `ExternalRace`).                   | [Clipboard and engine models](../formal/clipboard/README.md)                                                                                                                                  |
| Lean 4                      | Proves properties of codecs, text rendering, spoilers, and visibility. CI compares `lake exe vectors` with `formal/lean/vectors.json`, which the Rust codec tests also use. | `formal/lean`                                                                                                                                                                                 |

## Run the checks locally

Run the workspace checks from the repository root. The pinned toolchain includes `rustfmt`, `clippy`, `llvm-tools-preview`, and `wasm32-unknown-unknown`. Install `cargo-llvm-cov` for the coverage command.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check -p copied-codec --no-default-features --locked
cargo check --target wasm32-unknown-unknown -p copied-wasm --locked
cargo llvm-cov --workspace --summary-only
```

Install a nightly toolchain and run `cargo install cargo-fuzz` before using the fuzz targets. Put corpora and artifacts outside the repository.

```sh
cd fuzz
cargo +nightly fuzz list
cargo +nightly fuzz run convert_all_targets /path/to/corpus/convert_all_targets -- \
  -max_total_time=60 -len_control=0 -artifact_prefix=/path/to/artifacts/
```

TLA+ needs Java and `tla2tools.jar`. Lean uses the toolchain pinned in `formal/lean/lean-toolchain` via `elan`.

```sh
TLA2TOOLS_JAR=/path/to/tla2tools.jar COPIED_MD_FORMAL_OUT=/path/to/tmp sh formal/clipboard/check.sh
(cd formal/lean && lake build && lake exe vectors | diff -u vectors.json -)
```

Run `pnpm check` from the repository root to check the TypeScript workspace. Run the desktop UI and Rust app checks from `apps/desktop`.

```sh
pnpm --dir ui --ignore-workspace install --frozen-lockfile
pnpm --dir ui typecheck && pnpm --dir ui lint && pnpm --dir ui test && pnpm --dir ui build
cargo test --manifest-path src-tauri/Cargo.toml --locked
```
