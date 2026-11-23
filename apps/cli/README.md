# copied.md CLI

```sh
copied-md targets
copied-md convert --to rich-text --json < notes.md
copied-md convert --to html-source --file notes.md
copied-md mcp
```

The CLI reads stdin or `--file`. It writes the requested representation to stdout and diagnostics to stderr. The MCP server uses stdio and converts supplied Markdown locally. On macOS, install the `copied-md-pasteboard` helper and run `copied-md copy --clipboard --pick` to convert copied Markdown in place.
