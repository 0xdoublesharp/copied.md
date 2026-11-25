# copied.md CLI and MCP

Use `copied-md` to list formats, convert Markdown from stdin or a file, or copy the result to the system clipboard. Run these commands with either the native binary in `crates/copied-cli` or the Node package `@0xdoublesharp/copied-md-cli`.

```sh
copied-md targets
copied-md convert --to rich-text --json < notes.md
copied-md convert --to telegram < update.md
copied-md convert --to html-source --file notes.md
copied-md copy --to slack --clipboard
copied-md copy --clipboard --pick
copied-md mcp
```

`convert` writes the plain-text representation to stdout by default. Choose another available representation with `--format`, or use `--json` to get the full `schemaVersion: 1` payload with representations, metrics, and diagnostics. Without `--json`, conversion diagnostics go to stderr. `mcp` serves `list_copy_targets` and `convert_markdown` locally over stdio.

The native CLI writes to the macOS pasteboard, the Windows clipboard, and Linux sessions with Wayland data control or X11. With `copy --clipboard`, it checks the revision of the clipboard it read. If it detects a newer copy before writing, it exits with code `3`. A copy or symlink of the native binary named `copied-md-pasteboard` also exposes the helper commands used by the [VS Code extension](../vscode/README.md).

The Node CLI supports `copy` on macOS via `copied-md-pasteboard` on your `PATH`. Set `COPIED_MD_PASTEBOARD` to use a helper at a different path.
