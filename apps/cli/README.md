# copied.md CLI & MCP

`copied.md 2.0` provides a native Rust binary (`crates/copied-cli`, binary name `copied-md`) as well as the Node CLI package (`apps/cli`). Both expose the same commands and stdio MCP server.

```sh
copied-md targets
copied-md convert --to rich-text --json < notes.md
copied-md convert --to telegram < update.md
copied-md convert --to html-source --file notes.md
copied-md copy --to slack --clipboard
copied-md copy --clipboard --pick
copied-md mcp
```

- Reads stdin or `--file`, writes the requested representation to stdout, and prints diagnostics to stderr.
- `--json` emits the `schemaVersion: 1` payload (`representations`, `metrics`, `diagnostics`).
- `copy` writes directly to the native OS clipboard on macOS (`NSPasteboard`), Windows (`OpenClipboard` + `CF_HTML`), and Linux (Wayland / X11) with revision-checked stale-write protection (exit code `3`).
- `mcp` serves `list_copy_targets` and `convert_markdown` locally over stdio.
- When invoked via a copy or symlink named `copied-md-pasteboard`, the native binary acts as the drop-in pasteboard helper used by the VS Code extension (`copied.md.app/Contents/Helpers/copied-md-pasteboard`).
