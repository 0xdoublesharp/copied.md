# copied.md 2.0 feature comparison

This November 2026 comparison describes the workflows considered for the copied.md 2.0 release. The scores are the project’s assessments of the listed setups. The notes below describe where an extension, script, or extra setup is needed.

---

## Scores

Each of the 10 dimensions is scored **0–10** (10 = category-leading).

| Dimension (10 pts each)             | **copied.md v2.0** _(Rust + Tauri)_ | **Pandoc + Shell** | **Raycast + MD Ext.** | **PopClip + MD Ext.** | **copied.md v1.2** _(TS + Swift)_ | **Markdown Service Tools** | **MarkPaste / Web Tools** |
| :---------------------------------- | :---------------------------------: | :----------------: | :-------------------: | :-------------------: | :-------------------------------: | :------------------------: | :-----------------------: |
| 1. Destination Format Breadth       |               **10**                |         6          |           5           |           4           |                 8                 |             3              |             4             |
| 2. Chat & App Paste Fidelity        |               **10**                |         3          |           5           |           4           |                 6                 |             4              |             4             |
| 3. Reverse HTML → Markdown          |                **9**                |         8          |           7           |           7           |                 0                 |             6              |             4             |
| 4. Daily Workflow Speed & UX        |                **9**                |         4          |           8           |         **9**         |                 6                 |             5              |             3             |
| 5. Cross-Platform & Surfaces        |                **9**                |         8          |           6           |           3           |                 5                 |             3              |             7             |
| 6. Footprint & Performance          |                **9**                |         8          |           7           |         **9**         |                 5                 |             7              |             6             |
| 7. Security, Privacy & Verification |               **10**                |         9          |           7           |           8           |                 7                 |             8              |             4             |
| 8. Smart Context & Customization    |                **9**                |         7          |           7           |           6           |                 6                 |             4              |             3             |
| 9. CLI, MCP & Dev Ecosystem         |               **10**                |         8          |           4           |           3           |                 8                 |             4              |             1             |
| 10. Distribution & Packaging        |                **6**                |       **9**        |         **9**         |           8           |                 7                 |             7              |             8             |
| **Total Score ( / 100)**            |               **91**                |       **70**       |        **65**         |        **61**         |              **58**               |           **51**           |          **44**           |

## Features

**Yes** means the listed setup supports the feature. **Partial** means support is limited, needs additional setup, or was unverified on some operating systems. **No** means the listed setup lacks the feature.

### Formatting and paste

| Capability                                                | copied.md v2.0 (Rust)                   | copied.md v1.2 (TS/Swift)                            | Raycast + MD Ext.       | PopClip + MD Ext.       | Markdown Service Tools       | MarkPaste / Web Tools | Pandoc + Shell                       |
| :-------------------------------------------------------- | :-------------------------------------- | :--------------------------------------------------- | :---------------------- | :---------------------- | :--------------------------- | :-------------------- | :----------------------------------- |
| Rich Text (`text/html` + `text/plain`)                    | **Yes** (with `☑`/`☐` tasks)            | **Yes** (drops task checkboxes)                      | **Yes**                 | **Yes** (RTF)           | **Yes** (RTF via `textutil`) | **Yes**               | **Partial** (CLI only, no clipboard) |
| Slack Composer Paste (aligned tables, tasks)              | **Yes**                                 | **Yes**                                              | **Partial** (basic)     | **No**                  | **No**                       | **Partial**           | **No**                               |
| Slack API (`mrkdwn`, unescaped code tables)               | **Yes**                                 | **Partial** (`\|` bug in code tables)                | **Partial**             | **Partial**             | **No**                       | **No**                | **No**                               |
| Telegram (macOS native MD + Desktop HTML + spoilers)      | **Yes**                                 | **Partial** (macOS RTF dropped bold/code)            | **No**                  | **No**                  | **No**                       | **No**                | **No**                               |
| Signal Desktop (`text/signal` Chromium Pickle + spoilers) | **Yes**                                 | **Yes** (macOS/Chrome)                               | **No**                  | **No**                  | **No**                       | **No**                | **No**                               |
| Discord (H1–H3, aligned tables, `\|\|spoilers\|\|`)       | **Yes**                                 | **Partial** (no spoilers or aligned tables)          | **No**                  | **No**                  | **No**                       | **No**                | **No**                               |
| Reddit (Old + New 4-space code, safe `>!spoilers!<`)      | **Yes**                                 | **Partial** (fences broke Old Reddit)                | **No**                  | **No**                  | **No**                       | **No**                | **No**                               |
| Word & Teams (dark-mode safe contrast, GFM align)         | **Yes**                                 | **Partial** (hardcoded `#f4f4f4`, forced left align) | **Partial**             | **Partial** (RTF)       | **Partial** (RTF)            | **Partial**           | **Yes** (`.docx` export)             |
| WhatsApp (`*bold*`, `_italic_`, lists, quotes, code)      | **Yes**                                 | **Partial** (nested markers bug)                     | **No**                  | **No**                  | **No**                       | **No**                | **No**                               |
| X / Twitter (v3 weighted char counter + limit)            | **Yes**                                 | **Yes**                                              | **No**                  | **No**                  | **No**                       | **No**                | **No**                               |
| Facebook, SMS, HTML Source, Plain Text                    | **Yes**                                 | **Yes**                                              | **Partial**             | **Partial**             | **Partial**                  | **Partial**           | **Partial**                          |
| Reverse HTML → GFM Markdown                               | **Yes** (VS Code, Docs, MathML, tables) | **No**                                               | **Yes** (separate ext.) | **Yes** (separate ext.) | **Yes** (raw HTML)           | **Partial**           | **Yes**                              |
| Windows `CF_HTML` (`-1` no-context + framing)             | **Yes**                                 | **No**                                               | **Partial** (Win beta)  | **No**                  | **No**                       | **No**                | **No**                               |

### Desktop workflow

| Capability                                                   | copied.md v2.0 (Rust)                                            | copied.md v1.2 (TS/Swift)                | Raycast + MD Ext.               | PopClip + MD Ext.               | Markdown Service Tools    | MarkPaste / Web Tools | Pandoc + Shell |
| :----------------------------------------------------------- | :--------------------------------------------------------------- | :--------------------------------------- | :------------------------------ | :------------------------------ | :------------------------ | :-------------------- | :------------- |
| Global keyboard shortcut (customizable)                      | **Yes**                                                          | **Yes**                                  | **Yes**                         | **Partial** (selection trigger) | **Partial** (per-service) | **No**                | **No**         |
| Automatic destination app detection                          | **Yes** (8 app families where active-app detection is available) | **No**                                   | **No**                          | **No**                          | **No**                    | **No**                | **No**         |
| Per-app format pins + override manager UI                    | **Yes**                                                          | **Partial** (focused app only)           | **No**                          | **Partial** (enable/disable)    | **No**                    | **No**                | **No**         |
| Reuse original Markdown across apps                          | **Yes**                                                          | **No**                                   | **Partial** (clipboard history) | **No**                          | **No**                    | **No**                | **No**         |
| Floating picker (`1–9` + type-to-filter)                     | **Yes**                                                          | **Partial** (centered grid, arrows only) | **Yes**                         | **Yes** (cursor popover)        | **No**                    | **No**                | **No**         |
| Hover preview with character counts and limits               | **Yes**                                                          | **No**                                   | **No**                          | **No**                          | **No**                    | **Yes** (in browser)  | **No**         |
| Native copy notifications on macOS                           | **Yes**                                                          | **No** (modal alerts)                    | **Yes**                         | **Yes**                         | **No**                    | **No**                | **No**         |
| Convert selected text in one shortcut or paste automatically | **No**                                                           | **No**                                   | **Yes**                         | **Yes**                         | **Yes** (in-place)        | **No**                | **No**         |
| First-run interactive onboarding                             | **Yes**                                                          | **No**                                   | **Yes**                         | **Yes**                         | **No**                    | **N/A**               | **No**         |

### Platforms, tooling, and distribution

| Capability                                     | copied.md v2.0 (Rust)                               | copied.md v1.2 (TS/Swift)           | Raycast + MD Ext.               | PopClip + MD Ext. | Markdown Service Tools | MarkPaste / Web Tools | Pandoc + Shell                |
| :--------------------------------------------- | :-------------------------------------------------- | :---------------------------------- | :------------------------------ | :---------------- | :--------------------- | :-------------------- | :---------------------------- |
| Native Desktop OS Support                      | **macOS, Windows, Linux** (session limits on Linux) | macOS only                          | macOS + Win beta                | macOS only        | macOS only             | Browser only          | macOS / Win / Linux CLI       |
| Native copy without opening a WebView          | **Yes** (`CopyOnly`)                                | **No** (bundled Node)               | **Yes** (Swift host)            | **Yes**           | **Yes**                | **No**                | **Yes**                       |
| Native CLI + JSON schema + stdio MCP server    | **Yes** (single binary)                             | **Partial** (Node CLI + MCP)        | **No**                          | **No**            | **No**                 | **No**                | **Partial** (CLI, no MCP)     |
| Browser extension and VS Code                  | **Yes** (TypeScript converter)                      | **Yes** (TS core)                   | **No**                          | **No**            | **No**                 | **Partial**           | **No**                        |
| 100% Local / Offline + Strict URL Sanitization | **Yes**                                             | **Yes**                             | **Partial** (account/sync opt.) | **Yes**           | **Yes**                | **Partial**           | **Yes**                       |
| Formal Proofs (Lean 4 + TLA+) & Fuzzing        | **Yes** (59 Lean + 6 TLA+ + 7 fuzz targets)         | **Partial** (TLA+ + property tests) | **No**                          | **No**            | **No**                 | **No**                | **No**                        |
| License or pricing model                       | **Free (MIT OSS)**                                  | **Free (MIT OSS)**                  | Free / Pro                      | Paid / Setapp     | Free                   | Free / Freemium       | Free (GPL)                    |
| Signed/Notarized Installers & Auto-Update      | **Partial** (packaging configuration)               | **Partial** (unsigned macOS)        | **Yes**                         | **Yes**           | **Yes**                | **N/A**               | **Yes** (Homebrew/apt/winget) |

## Items to verify for releases

- Signed and notarized universal macOS downloads, Homebrew distribution, Mac App Store builds, the updater, and Windows and Linux installers. [Packaging guide](app-store-and-packaging.md)
- Live clipboard and shortcut behavior on Windows 11 and Wayland.
- Reverse HTML conversion with live clipboard samples from Notion, Slack, ChatGPT, and Claude. [Paste checklist](paste-fidelity.md)

The 2.0 comparison also identified two workflow gaps: converting selected text or pasting in one shortcut, and detecting the active web app inside a browser.
