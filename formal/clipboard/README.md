# macOS clipboard protocol

The model follows one copy operation through clipboard capture, format selection, conversion, preparation, and write. Up to three copies from other applications can occur at any step. The revision abstracts AppKit’s change count. Each external copy and each call to clear the pasteboard advances it.

The main configuration checks that cancelling, conversion failure, and a stale revision detected before clearing leave the clipboard alone. It also checks that copied.md skips the write if a new copy is visible after clearing, leaving that copy intact.

Use a JDK and the official [TLA+ tools release](https://github.com/tlaplus/tlaplus/releases/tag/v1.7.4). From the repository root, run:

```sh
TLA2TOOLS_JAR=/path/to/tla2tools.jar pnpm test:formal
```

You can also run `sh formal/clipboard/check.sh` with the same environment variable. Set `COPIED_MD_FORMAL_OUT` to a disposable directory on machines without `/Volumes/Scratch`.

The script checks the current protocol and requires two expected counterexamples:

- `LegacyClipboard.cfg` reproduces a new copy arriving after the clear. The old protocol proceeds to write despite the changed revision.
- `ExternalRace.cfg` demonstrates the remaining race. AppKit exposes separate operations for a revision read, a clear, and a write. Another application can still copy between those calls.

The Swift regression test reproduces the legacy interleaving through the same pasteboard write function the app uses. The TLC model is bounded to one operation and does not verify AppKit itself.
