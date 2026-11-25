# Tauri and Rust architecture

copied.md 2 moves conversion, clipboard access, and the desktop app into a Rust workspace. The browser extension and VS Code extension can call the same converter through WebAssembly. The desktop app is a Tauri v2 tray app that uses the same crates.

## Crates

| Crate              | Role                                                                                                                                                                                                                           | Platform code                                                                                     |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------- |
| `copied-codec`     | Clipboard wire formats: Chromium Pickle custom data (Signal), the Windows `CF_HTML` envelope, and ASCII-safe RTF escaping. `no_std + alloc`, `forbid(unsafe_code)`.                                                            | None                                                                                              |
| `copied-core`      | Parses GitHub Flavored Markdown and renders the 15 copy targets. Returns the `schemaVersion: 1` payload (`text/plain`, `text/html`, `text/rtf`, `text/signal`) with metrics and diagnostics. Owns the URL policy (`safe_url`). | None                                                                                              |
| `copied-config`    | Versioned desktop settings (`ConfigV2`), format visibility, per-app pins, shortcut parsing, legacy macOS preference migration, and atomic persistence.                                                                         | None                                                                                              |
| `copied-engine`    | The copy session state machine: copy-and-pick, picker, and copy-only modes, monotone picker IDs, stale-reply handling, and shortcut rebinding.                                                                                 | None                                                                                              |
| `copied-clipboard` | Revision-checked clipboard reads and writes. A write aborts when another app copied after the session began.                                                                                                                   | macOS `NSPasteboard`, Windows `OpenClipboard`, Linux Wayland data-control and X11 selections      |
| `copied-platform`  | Global shortcuts, active-app detection, launch at login, and a capability report per session.                                                                                                                                  | macOS `SMAppService`, Windows registry and hotkeys, Linux X11 hotkeys, XDG autostart, and portals |
| `copied-cli`       | The native `copied-md` command line and stdio MCP server.                                                                                                                                                                      | Uses `copied-clipboard`                                                                           |
| `copied-wasm`      | `wasm-bindgen` facade over `copied-core` for the browser and VS Code extensions.                                                                                                                                               | `wasm32-unknown-unknown`                                                                          |

The desktop app lives in `apps/desktop`. `src-tauri` is a separate Cargo package with its own lockfile and is excluded from the root workspace, as is `fuzz/`. The Vue picker and settings UI in `apps/desktop/ui` has its own pnpm lockfile.

The four pure crates (`codec`, `core`, `config`, `engine`) contain no platform code, so their tests run the same on every OS and their coverage is gated in CI.

## Destination formatting

`copied-core` reproduces the frozen `ts-oracle-v1` TypeScript engine byte for byte through `copied_core::oracle::convert_markdown_ts_v1`. The default `convert_markdown` adds seven documented improvements, listed in `crates/copied-core/tests/parity/divergences.toml`:

| Improvement                                                            | Targets                               |
| ---------------------------------------------------------------------- | ------------------------------------- |
| No repeated bold, italic, or strike markers inside the same style      | Slack API, WhatsApp, Telegram, Signal |
| Native Telegram `<tg-spoiler>` that never leaks into plain text or RTF | Telegram                              |
| Discord `\|\|spoilers\|\|` and Reddit `>!spoilers!<` that fail closed  | Discord, Reddit                       |
| Aligned columns for tables shown in monospace                          | Slack paste, Discord, WhatsApp        |
| Styled block quotes and a subtle code background                       | Teams, Word                           |
| Visible checkbox glyphs for task lists                                 | Slack paste, Teams, Word              |
| Raw HTML blocks kept as separate Markdown blocks                       | Discord, Reddit                       |

The other targets (Rich Text, Plain Text, HTML Source, Facebook, X, and SMS) match the oracle exactly. X reports weighted length against the 280-character limit. Every target shares the URL policy: fragments, `http:`, `https:`, and `mailto:` survive; relative links resolve against `sourceUrl`; every other scheme is dropped with an `unsupported-url` diagnostic.

## Verification

Each layer checks a different kind of mistake.

| Layer                       | What it checks                                                                                                                                                                                                                                                       | Where                                                                                          |
| --------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Golden parity               | Oracle mode matches the TypeScript corpus, and the improved output matches its own golden file.                                                                                                                                                                      | `crates/copied-core/tests/parity.rs`, `tests/golden/corpus.json`, `tests/parity/improved.json` |
| Differential corpus         | Oracle mode against a large generated TypeScript corpus. The merge in commit `077b8dd` matched 56,850 of 56,850 conversions. The test is `#[ignore]` and needs `COPIED_ORACLE_JSON`.                                                                                 | `crates/copied-core/tests/differential.rs`                                                     |
| Coverage                    | 100% line coverage for `copied-codec`, `copied-config`, and `copied-engine`, and at least 98% for `copied-core`.                                                                                                                                                     | CI `rust` job on Ubuntu                                                                        |
| Property tests (`proptest`) | Payload, metric, and determinism invariants across all targets; safe HTML; balanced RTF; fail-closed spoilers; codec round trips; config invariants; engine state machine.                                                                                           | `crates/*/tests`, `crates/copied-config/src/tests.rs`                                          |
| `cargo-fuzz`                | Seven libFuzzer targets: `convert_all_targets`, `safe_url`, `pickle_decode`, `cf_html_parse`, `rtf_roundtrip`, `config_migrate`, and `shortcut_parse`. They share the property-test oracles in `fuzz/src/lib.rs`.                                                    | `fuzz/`                                                                                        |
| TLA+ (TLC)                  | Six configurations. Four pass (`Clipboard`, `WinBoard`, `CopyAndPick`, `Engine`), and two reproduce expected counterexamples (`LegacyClipboard`, `ExternalRace`) that show why the macOS post-clear revision check is needed.                                        | `formal/clipboard`                                                                             |
| Lean 4                      | 59 theorems about the codecs, text rendering, spoilers, and visibility, with no `sorry`. `lake exe vectors` prints test vectors from the proven functions; `crates/copied-codec/tests/lean_vectors.rs` checks the Rust codecs against the checked-in `vectors.json`. | `formal/lean`                                                                                  |

## Run the checks locally

Run these from the repository root. The pinned toolchain in `rust-toolchain.toml` installs `rustfmt`, `clippy`, `llvm-tools-preview`, and the `wasm32-unknown-unknown` target.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check -p copied-codec --no-default-features
cargo check --target wasm32-unknown-unknown -p copied-wasm
cargo llvm-cov --workspace --summary-only
```

Fuzzing needs a nightly toolchain and `cargo install cargo-fuzz`. Keep the corpus and any crash artifacts outside the repository:

```sh
cd fuzz
cargo +nightly fuzz list
cargo +nightly fuzz run convert_all_targets /path/to/corpus/convert_all_targets -- \
  -max_total_time=60 -len_control=0 -artifact_prefix=/path/to/artifacts/
```

`convert_all_targets` runs about 150–300 inputs per second because each input converts every target twice. Use `-len_control=0` so short runs still try full-length documents.

TLA+ needs Java and `tla2tools.jar`; CI pins v1.7.4 by SHA-256. Lean needs `elan`, which installs the toolchain named in `formal/lean/lean-toolchain`.

```sh
TLA2TOOLS_JAR=/path/to/tla2tools.jar COPIED_MD_FORMAL_OUT=/path/to/tmp sh formal/clipboard/check.sh
(cd formal/lean && lake build && lake exe vectors | diff -u vectors.json -)
```

The desktop app builds from `apps/desktop`:

```sh
pnpm --dir ui --ignore-workspace install --frozen-lockfile
pnpm --dir ui test
pnpm --dir ui build
cargo test --manifest-path src-tauri/Cargo.toml
```
