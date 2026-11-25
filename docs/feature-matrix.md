# copied.md v2.0 (Rust + Tauri) — Competitive Feature Matrix & Scorecard

> Last updated: November 2026 · Branch: `migration/tauri-rust`

This matrix compares the new **Rust + Tauri v2** version of `copied.md` against our previous TypeScript + Swift baseline (`v1.2`) and the primary alternatives users reach for when moving Markdown between editors, AI tools, chat apps, and documents.

---

## Overall Weighted Score (100 Points)

Each of the 10 dimensions is scored **0–10** (10 = category-leading).

| Dimension (10 pts each) | **copied.md v2.0** *(Rust + Tauri)* | **Pandoc + Shell** | **Raycast + MD Ext.** | **PopClip + MD Ext.** | **copied.md v1.2** *(TS + Swift)* | **Markdown Service Tools** | **MarkPaste / Web Tools** |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| 1. Destination Format Breadth | **10** | 6 | 5 | 4 | 8 | 3 | 4 |
| 2. Chat & App Paste Fidelity | **10** | 3 | 5 | 4 | 6 | 4 | 4 |
| 3. Reverse HTML → Markdown | **9** | 8 | 7 | 7 | 0 | 6 | 4 |
| 4. Daily Workflow Speed & UX | **9** | 4 | 8 | **9** | 6 | 5 | 3 |
| 5. Cross-Platform & Surfaces | **9** | 8 | 6 | 3 | 5 | 3 | 7 |
| 6. Footprint & Performance | **9** | 8 | 7 | **9** | 5 | 7 | 6 |
| 7. Security, Privacy & Verification | **10** | 9 | 7 | 8 | 7 | 8 | 4 |
| 8. Smart Context & Customization | **9** | 7 | 7 | 6 | 6 | 4 | 3 |
| 9. CLI, MCP & Dev Ecosystem | **10** | 8 | 4 | 3 | 8 | 4 | 1 |
| 10. Distribution & Packaging | **6** | **9** | **9** | 8 | 7 | 7 | 8 |
| **Total Score ( / 100)** | **91** | **70** | **65** | **61** | **58** | **51** | **44** |

**Delta vs. `copied.md v1.2`:** **+33 points** (`58 → 91`), driven by native Rust + Tauri v2 cross-platform architecture, reverse HTML → Markdown, Telegram macOS / Slack API / Old Reddit / Word & Teams / Rich Text paste fidelity fixes, Spotlight-grade UX (hover-to-preview, `1–9` keys, type-to-filter, active-monitor floating panel, non-blocking native HUD toasts, auto-detect destination apps, last-Markdown reuse), and Lean 4 + TLA+ + `cargo-fuzz` verification.

---

## Detailed Feature Comparison

Legend: **Yes** = built-in & verified · **Partial** = limited, requires separate plugins/scripts, or unverified on some OSes · **No** = unsupported.

### Core Formatting & Destination Fidelity

| Capability | copied.md v2.0 (Rust) | copied.md v1.2 (TS/Swift) | Raycast + MD Ext. | PopClip + MD Ext. | Markdown Service Tools | MarkPaste / Web Tools | Pandoc + Shell |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| Rich Text (`text/html` + `text/plain`) | **Yes** (with `☑`/`☐` tasks) | **Yes** (drops task checkboxes) | **Yes** | **Yes** (RTF) | **Yes** (RTF via `textutil`) | **Yes** | **Partial** (CLI only, no clipboard) |
| Slack Composer Paste (aligned tables, tasks) | **Yes** | **Yes** | **Partial** (basic) | **No** | **No** | **Partial** | **No** |
| Slack API (`mrkdwn`, unescaped code tables) | **Yes** | **Partial** (`\|` bug in code tables) | **Partial** | **Partial** | **No** | **No** | **No** |
| Telegram (macOS native MD + Desktop HTML + spoilers) | **Yes** | **Partial** (macOS RTF dropped bold/code) | **No** | **No** | **No** | **No** | **No** |
| Signal Desktop (`text/signal` Chromium Pickle + spoilers) | **Yes** | **Yes** (macOS/Chrome) | **No** | **No** | **No** | **No** | **No** |
| Discord (H1–H3 clamp, aligned tables, `||spoilers||`) | **Yes** | **Partial** (no spoilers / unaligned tables) | **No** | **No** | **No** | **No** | **No** |
| Reddit (Old + New 4-space code, safe `>!spoilers!<`) | **Yes** | **Partial** (fences broke Old Reddit) | **No** | **No** | **No** | **No** | **No** |
| Word & Teams (dark-mode safe contrast, GFM align) | **Yes** | **Partial** (hardcoded `#f4f4f4`, forced left align) | **Partial** | **Partial** (RTF) | **Partial** (RTF) | **Partial** | **Yes** (`.docx` export) |
| WhatsApp (`*bold*`, `_italic_`, lists, quotes, code) | **Yes** | **Partial** (nested markers bug) | **No** | **No** | **No** | **No** | **No** |
| X / Twitter (v3 weighted char counter + limit) | **Yes** | **Yes** | **No** | **No** | **No** | **No** | **No** |
| Facebook, SMS, HTML Source, Plain Text | **Yes** | **Yes** | **Partial** | **Partial** | **Partial** | **Partial** | **Partial** |
| Reverse HTML → GFM Markdown | **Yes** (VS Code, Docs, MathML, tables) | **No** | **Yes** (separate ext.) | **Yes** (separate ext.) | **Yes** (raw HTML) | **Partial** | **Yes** |
| Windows `CF_HTML` (`-1` no-context + framing) | **Yes** | **No** | **Partial** (Win beta) | **No** | **No** | **No** | **No** |

### Desktop UX, Speed & Intelligence

| Capability | copied.md v2.0 (Rust) | copied.md v1.2 (TS/Swift) | Raycast + MD Ext. | PopClip + MD Ext. | Markdown Service Tools | MarkPaste / Web Tools | Pandoc + Shell |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| Global keyboard shortcut (customizable) | **Yes** | **Yes** | **Yes** | **Partial** (selection trigger) | **Partial** (per-service) | **No** | **No** |
| Automatic destination app detection | **Yes** (8 apps, default-on) | **No** | **No** | **No** | **No** | **No** | **No** |
| Per-app format pins + override manager UI | **Yes** | **Partial** (focused app only) | **No** | **Partial** (enable/disable) | **No** | **No** | **No** |
| Last-Markdown source memory across apps | **Yes** | **No** | **Partial** (clipboard history) | **No** | **No** | **No** | **No** |
| Floating active-monitor picker (`1–9` + type-to-filter) | **Yes** | **Partial** (centered grid, arrows only) | **Yes** | **Yes** (cursor popover) | **No** | **No** | **No** |
| Hover-to-preview with char limits & calm diagnostics | **Yes** | **No** | **No** | **No** | **No** | **Yes** (in browser) | **No** |
| Non-blocking native HUD toast (zero WebView) | **Yes** | **No** (modal alerts) | **Yes** | **Yes** | **No** | **No** | **No** |
| 1-chord copy selection / auto-paste into app | **No** *(planned opt-in)* | **No** | **Yes** | **Yes** | **Yes** (in-place) | **No** | **No** |
| First-run interactive onboarding | **Yes** | **No** | **Yes** | **Yes** | **No** | **N/A** | **No** |

### Architecture, Verification & Ecosystem

| Capability | copied.md v2.0 (Rust) | copied.md v1.2 (TS/Swift) | Raycast + MD Ext. | PopClip + MD Ext. | Markdown Service Tools | MarkPaste / Web Tools | Pandoc + Shell |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| Native Desktop OS Support | **macOS** (live) · **Win/Linux** (built, installers pending) | macOS only | macOS + Win beta | macOS only | macOS only | Browser only | macOS / Win / Linux CLI |
| Zero-WebView `CopyOnly` & ~16 MB native bundle | **Yes** (`15.8 MiB`) | **No** (bundled Node) | **Yes** (Swift host) | **Yes** | **Yes** | **No** | **Yes** |
| Native CLI + JSON schema + stdio MCP server | **Yes** (single binary) | **Partial** (Node CLI + MCP) | **No** | **No** | **No** | **No** | **Partial** (CLI, no MCP) |
| Browser Extension + VS Code + WASM core | **Yes** (`copied-wasm` ready) | **Yes** (TS core) | **No** | **No** | **No** | **Partial** | **Partial** (WASM build) |
| 100% Local / Offline + Strict URL Sanitization | **Yes** | **Yes** | **Partial** (account/sync opt.) | **Yes** | **Yes** | **Partial** | **Yes** |
| Formal Proofs (Lean 4 + TLA+) & Fuzzing | **Yes** (59 Lean + 6 TLA+ + 7 fuzz targets) | **Partial** (TLA+ + property tests) | **No** | **No** | **No** | **No** | **No** |
| Price / License | **Free (MIT OSS)** | **Free (MIT OSS)** | Free / Pro ($8+/mo) | Paid ($12–$25 / Setapp) | Free | Free / Freemium | Free (GPL) |
| Signed/Notarized Installers & Auto-Update | **Pending** *(biggest remaining gap)* | **Partial** (unsigned macOS) | **Yes** | **Yes** | **Yes** | **N/A** | **Yes** (Homebrew/apt/winget) |

---

## Where `copied.md v2.0` Wins — and the Remaining 9 Points

### Unmatched Strengths
- **Only tool built specifically for real-world app paste quirks:** native Signal `text/signal` Chromium Pickle, Telegram macOS Markdown (working around `telegram-macos`'s broken RTF attribute importer), Slack Paste aligned tables + `mrkdwn` API output, Old + New Reddit 4-space code blocks, dark-mode-safe Word/Teams HTML, and visible `☑`/`☐` task lists across Gmail/Docs/Mail/Notes/Notion.
- **Zero-friction multi-app loop:** copy Markdown once, switch to Slack (`⌃⌘C` auto-detects Slack), switch to Telegram (`⌃⌘C` reuses original Markdown and auto-detects Telegram), or hover-preview all 15 targets in a Spotlight-style floating panel.
- **Developer & AI-native:** single native Rust binary providing CLI, structured JSON + diagnostics, stdio MCP server, VS Code pasteboard helper, and `copied-wasm`—backed by 59 Lean 4 theorems, 6 TLA+ specs, 7 libFuzzer targets, and 100% coverage on codec/config/engine crates.

### Remaining Gaps to Reach ~98–100 / 100
1. **Distribution & Trust (-4 pts):** Ship notarized Developer ID `.dmg`, Homebrew cask (`brew install --cask copied-md`), Tauri auto-updater, Mac App Store sandboxed build, and verified Windows/Linux installers.
2. **1-Chord Selection / Auto-Paste (-1 pt):** Optional direct-download Accessibility mode to convert selected text or paste immediately on pick (matching PopClip / Raycast speed).
3. **Browser Web-App Auto-Detection (-1 pt):** Detect Slack, Notion, Gmail, GitHub, Reddit, and X inside Chrome/Arc/Safari/Firefox.
4. **Live Windows & Wayland Verification (-1 pt):** Run end-to-end paste matrix on native Windows 11 and Wayland desktop portals.
5. **Universal macOS & Live Reverse Fixtures (-2 pts):** Ship universal (`arm64` + `x86_64`) signed build and verify reverse HTML → Markdown fixtures from live Notion, Slack, ChatGPT, and Claude copy actions.
