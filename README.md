# copied.md

Copy Markdown as rich text, plain text, HTML source, or a destination-specific format. Conversion runs locally.

## Browser and VS Code

Unzip the Chrome or Edge package, enable developer mode on the browser's extensions page, and choose **Load unpacked**. For temporary Firefox testing, open `about:debugging` and load the extension from its extracted manifest. In VS Code, choose **Extensions: Install from VSIX…** and select the packaged file.

In Chrome, Edge, or Firefox, select Markdown and choose **Copy Markdown As…** from the context menu. The popup also accepts pasted Markdown and shows the plain-text representation before copying. Assign a shortcut to **Copy selection using the last format** in the browser's extension shortcuts.

In VS Code, run **Copy Markdown As…** from the command palette. The picker copies selected text in document order, or offers the entire document when a Markdown file has no selection. VS Code can always use its text clipboard API. To copy rich text from a local macOS VS Code window, set `copied-md.pasteboardHelper` to the installed macOS helper's absolute path.

**Rich Text** provides HTML and readable plain text. **HTML Source** copies markup as text. **Slack** is for pasting into the composer. **Slack API (mrkdwn)** is for API payloads. Clipboard format support depends on the receiving app.

## Command line

Requires Node 24.15 or later in the Node 24 LTS line, or Node 26 and later. Install the packed CLI with `npm install -g ./0xdoublesharp-copied-md-cli-1.0.0.tgz`.

```sh
copied-md targets
copied-md convert --to html-source --file notes.md
copied-md convert --to rich-text --json < notes.md
copied-md convert --to slack-api < update.md
```

Conversion writes the requested representation to stdout. `--json` returns the representations, character counts, and diagnostics together. CLI errors use stderr and a nonzero exit code. `--source-url` resolves relative links and `--unicode` enables decorative Unicode for text destinations that support it.

## macOS and T3 Code

Open **copied.md.app** once. Its clipboard icon appears in the menu bar and the app registers **Control–Option–Command–M** (`⌃⌥⌘M`). The app includes its converter and runs on macOS 13.5 or later on Apple silicon and Intel.

1. Click **Copy Message** on a reply in T3 Code, or copy Markdown in any app.
2. Press `⌃⌥⌘M` to open **Copy Markdown As…**.
3. Choose a format and paste into the destination.

The clipboard menu also opens the picker and lets you change or disable the keyboard shortcut. Enable **Launch at Login** there if you want copied.md already running when you sign in. The shortcut is available while copied.md is running.

To build the app and a ZIP, run `pnpm build:macos-app` from a dependency-installed Scratch source copy. The build uses `/Volumes/Scratch/copied-md/build/portable` by default. Set `COPIED_MD_APP_OUT` to build in another output directory. A developer build uses a local ad-hoc signature. Distributable downloads require Developer ID signing and notarization.

### macOS command line

Build the local helper with `pnpm build:macos` and install the printed executable as `copied-md-pasteboard`. The CLI resolves it through `PATH` or the `COPIED_MD_PASTEBOARD` environment variable.

Use `copied-md copy --clipboard --pick` to invoke the picker from a terminal or a script. The helper checks whether the clipboard changed while you chose a format.

## MCP

Configure your agent to launch the installed CLI on the machine where that agent runs. The stdio server exposes `list_copy_targets` and `convert_markdown`. It operates on Markdown supplied in the tool call.

```json
{
  "mcpServers": {
    "copied-md": {
      "command": "/absolute/path/to/copied-md",
      "args": ["mcp"]
    }
  }
}
```

For T3 Code, add the server through the chosen provider's MCP configuration and restart the agent session to reload tools. Clipboard actions run separately on your Mac.

## Develop and package

Use the pnpm version pinned in `package.json`. For local disposable builds, run the install and build in a source copy on Scratch.

```sh
pnpm install --frozen-lockfile
pnpm check
pnpm build:all
pnpm zip
pnpm zip:edge
pnpm zip:firefox
pnpm package:vscode
pnpm package:cli
```

Browser archives and VSIX packages can be tested locally. Firefox release-channel installation requires Mozilla signing.
