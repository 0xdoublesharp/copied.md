# Tauri and Rust architecture

copied.md 2 moves conversion, clipboard access, reverse HTML → Markdown conversion, and the desktop app into an 8-crate Rust workspace. The browser extension and VS Code extension can call the same converter through WebAssembly (`copied-wasm`). The desktop app is a Tauri v2 tray app backed by the same crates.

## Crates

| Crate              | Role                                                                                                                                                                                                                                  | Platform code                                                                                     |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `copied-codec`     | Clipboard wire formats: Chromium Pickle custom data (Signal), Windows `CF_HTML` encoder and parser (including Microsoft's `-1` no-context envelope), and ASCII-safe RTF escaping. `no_std + alloc`, `forbid(unsafe_code)`.            | None                                                                                              |
| `copied-core`      | Parses GitHub Flavored Markdown into 15 destination targets (`schemaVersion: 1` payload with metrics and diagnostics), owns the URL policy (`safe_url`), and provides reverse `html_to_markdown` (VS Code, Docs, MathML, GFM tables). | None                                                                                              |
| `copied-config`    | Versioned desktop settings (`ConfigV2`), format visibility, built-in destination app auto-detection, per-app pins, shortcut parsing, legacy macOS preference migration, and atomic persistence.                                       | None                                                                                              |
| `copied-engine`    | Copy session state machine: copy-and-pick, picker, and copy-only modes, monotone picker IDs, stale-reply handling, last-Markdown source reuse across apps, live preview snapshots, reverse HTML → Markdown, and shortcut rebinding.   | None                                                                                              |
| `copied-clipboard` | Revision-checked clipboard text/HTML reads and multi-flavor writes. Aborts writes if another app copied after the session began.                                                                                                      | macOS `NSPasteboard`, Windows `OpenClipboard`, Linux Wayland data-control and X11 selections      |
| `copied-platform`  | Global shortcuts, active-app detection, launch at login, and a capability report per session.                                                                                                                                         | macOS `SMAppService`, Windows registry and hotkeys, Linux X11 hotkeys, XDG autostart, and portals |
| `copied-cli`       | Native `copied-md` CLI, cross-platform clipboard commands, stdio MCP server, and `copied-md-pasteboard` alias.                                                                                                                        | Uses `copied-clipboard`                                                                           |
| `copied-wasm`      | `wasm-bindgen` facade over `copied-core` for the browser and VS Code extensions.                                                                                                                                                      | `wasm32-unknown-unknown`                                                                          |

The desktop app lives in `apps/desktop`. `src-tauri` is a separate Cargo package with its own lockfile and is excluded from the root workspace, as is `fuzz/`. The Vue 3 picker, settings, and onboarding UI in `apps/desktop/ui` has its own pnpm lockfile.

The four pure crates (`codec`, `core`, `config`, `engine`) contain no platform code, so their tests run identically on every OS and their coverage is gated in CI.

## Desktop UX architecture

- **Zero-WebView `CopyOnly` path:** `apps/desktop/src-tauri` starts as a tray accessory with zero open WebViews. Success, warning, and error feedback use a reusable native macOS `NSPanel` HUD (`src/toast.rs`, with tray tooltip fallback on other OSes) guarded by an atomic generation counter so repeated notifications replace in place and never steal focus.
- **Spotlight-style floating picker:** `src/picker.rs` creates one reusable WebView on demand, floats it on all spaces with a macOS overlay titlebar, and positions it on whichever monitor contains the pointer. The Vue UI adds `1–9` / `⌘1–⌘9` quick-select keys, instant type-to-filter, hover-to-preview, active-app badges, saved override management, and a first-run interactive Welcome panel.

## Destination formatting

`copied-core` reproduces the frozen `ts-oracle-v1` TypeScript engine byte for byte through `copied_core::oracle::convert_markdown_ts_v1`. The public `convert_markdown` adds eleven documented improvements in `crates/copied-core/tests/parity/divergences.toml`:

| Improvement                                                                       | Targets                               |
| --------------------------------------------------------------------------------- | ------------------------------------- |
| No repeated bold, italic, or strike markers inside the same style                 | Slack API, WhatsApp, Telegram, Signal |
| Telegram plain text emits native Telegram Markdown (splitting bold around code)   | Telegram                              |
| Telegram omits broken RTF flavor so Telegram for macOS preserves bold/italic/code | Telegram                              |
| Native Telegram `<tg-spoiler>` and `\|\|spoiler\|\|` with fail-closed fallback    | Telegram                              |
| Discord `\|\|spoilers\|\|` and Reddit `>!spoilers!<` with fail-closed fallback    | Discord, Reddit                       |
| Aligned columns for tables shown in monospace                                     | Slack paste, Discord, WhatsApp        |
| Slack API table cells keep literal `\|` inside fenced code blocks                 | Slack API                             |
| Dark-mode-safe code/header/quote contrast and preserved GFM header alignment      | Teams, Word                           |
| Visible `☑` / `☐` checkbox glyphs for task lists                                  | Rich Text, Slack paste, Teams, Word   |
| Raw HTML blocks kept as separate Markdown blocks                                  | Discord, Reddit                       |
| Four-space-indented code blocks compatible with both Old and New Reddit           | Reddit                                |

Every target shares the URL policy (`safe_url`): fragments, `http:`, `https:`, and `mailto:` survive; relative links resolve against `sourceUrl`; every other scheme is omitted cleanly with an `unsupported-url` diagnostic.

## Verification

Each layer checks a different class of failure.

| Layer                       | What it checks                                                                                                                                                                                                  | Where                                                                                          |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Golden parity               | Oracle mode matches 960/960 frozen TypeScript outputs, and improved output matches its reviewed golden file with zero stale/undocumented divergences.                                                           | `crates/copied-core/tests/parity.rs`, `tests/golden/corpus.json`, `tests/parity/improved.json` |
| Differential corpus         | Oracle mode against a generated TypeScript corpus (56,850/56,850 conversions matched). Ignored by default (`COPIED_ORACLE_JSON`).                                                                               | `crates/copied-core/tests/differential.rs`                                                     |
| Coverage                    | 100% line coverage for `copied-codec`, `copied-config`, and `copied-engine`, and at least 98% for `copied-core`.                                                                                                | CI `rust` job on Ubuntu                                                                        |
| Property tests (`proptest`) | Payload, metric, and determinism invariants across all targets; safe HTML; balanced RTF; fail-closed spoilers; codec round trips; config invariants; engine state machine.                                      | `crates/*/tests`, `crates/copied-config/src/tests.rs`                                          |
| `cargo-fuzz`                | Seven libFuzzer targets: `convert_all_targets`, `safe_url`, `pickle_decode`, `cf_html_parse`, `rtf_roundtrip`, `config_migrate`, and `shortcut_parse`.                                                          | `fuzz/`                                                                                        |
| TLA+ (TLC)                  | Six configurations: four passing (`Clipboard`, `WinBoard`, `CopyAndPick`, `Engine`) and two expected counterexamples (`LegacyClipboard`, `ExternalRace`).                                                       | `formal/clipboard`                                                                             |
| Lean 4                      | 59 theorems about codecs, text rendering, spoilers, and visibility (zero `sorry`). `lake exe vectors` checks proven outputs against `formal/lean/vectors.json` and `crates/copied-codec/tests/lean_vectors.rs`. | `formal/lean`                                                                                  |

## Run the checks locally

Run these from the repository root. The pinned toolchain in `rust-toolchain.toml` installs `rustfmt`, `clippy`, `llvm-tools-preview`, and `wasm32-unknown-unknown`.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check -p copied-codec --no-default-features --locked
cargo check --target wasm32-unknown-unknown -p copied-wasm --locked
cargo llvm-cov --workspace --summary-only
```

Fuzzing needs a nightly toolchain and `cargo install cargo-fuzz`. Keep corpora and artifacts outside the repository:

```sh
cd fuzz
cargo +nightly fuzz list
cargo +nightly fuzz run convert_all_targets /path/to/corpus/convert_all_targets -- \
  -max_total_time=60 -len_control=0 -artifact_prefix=/path/to/artifacts/
```

TLA+ needs Java and `tla2tools.jar`; Lean needs `elan` for the toolchain pinned in `formal/lean/lean-toolchain`:

```sh
TLA2TOOLS_JAR=/path/to/tla2tools.jar COPIED_MD_FORMAL_OUT=/path/to/tmp sh formal/clipboard/check.sh
(cd formal/lean && lake build && lake exe vectors | diff -u vectors.json -)
```

The desktop app builds and tests from `apps/desktop`:

```sh
pnpm --dir ui --ignore-workspace install --frozen-lockfile
pnpm --dir ui typecheck && pnpm --dir ui lint && pnpm --dir ui test && pnpm --dir ui build
cargo test --manifest-path src-tauri/Cargo.toml --locked
```
