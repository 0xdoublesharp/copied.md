# Clipboard and desktop engine models

Run all six TLC configurations from the repository root.

```sh
sh formal/clipboard/check.sh
```

The script looks for `java` on `PATH`, then for Homebrew OpenJDK at
`/opt/homebrew/opt/openjdk/bin/java`. It uses `TLA2TOOLS_JAR` when set and
defaults to `~/.local/share/tla2tools/tla2tools.jar`. Get the jar from the
[official TLA+ tools releases](https://github.com/tlaplus/tlaplus/releases).
`pnpm test:formal` runs the same script. On machines without a Scratch
volume, set `COPIED_MD_FORMAL_OUT` to a disposable output directory.

## Native clipboard protocols

Both platform models follow one copy operation and allow up to three external
copies. TLC checks the modeled interleavings under each platform's clipboard
contract.

| Configuration | Expected result | What it checks |
| --- | --- | --- |
| `Clipboard.cfg` | Pass | Cancellation, conversion failure, and a stale revision detected before clear preserve the clipboard. If the post-clear revision check observes a newer copy, the write aborts and preserves it. |
| `LegacyClipboard.cfg` | Counterexample to `NeverWriteAfterAnObservedChange` | With the post-clear guard disabled, an external copy can be observed and then overwritten. The Swift regression test reproduces this interleaving through the app's pasteboard write routine. |
| `ExternalRace.cfg` | Counterexample to `NoExternalRace` | AppKit exposes separate revision reads, clear, and write calls. An external copy can land between a check and the next mutation. |
| `WinBoard.cfg` | Pass, including `NoExternalRace` | `OpenClipboard` gives one writer exclusive access. The engine compares `GetClipboardSequenceNumber` while holding that lock and keeps it through clear and write. A stale comparison releases the lock with the external copy intact. |

## Copy session

`CopyAndPick.cfg` checks a captured session in `CopyAndPick`, `Picker`, or
`CopyOnly` mode. The clipboard carries `source`, `external`, `empty`, or
`conv(target, src)` and can receive up to three external copies after capture.
The term `source` means the text captured at the start of this session.

- `NeverConvertAConversion` checks that every successful write uses the
  captured source, including a replacement for the automatic copy.
- `SecondWriteChecksFirstRevision` checks that the replacement expects the
  revision returned by the automatic write.
- `ExternalCopyBetweenWritesAbortsCleanly` checks that a replacement after an
  external copy becomes stale and leaves the external payload intact.
- `CancelAfterAutoCopyPreservesAutoCopy` checks that cancellation performs no
  write. The automatic copy remains until another application copies.
- `PinnedAutoCopyNeverRewritesLastTarget` checks that an automatic copy driven
  by a per-application pin preserves the remembered global target.

## Desktop lifecycle

`Engine.cfg` explores up to three picker generations across copy sessions and
models shortcut recording, cancellation, rebinding, and toggling. It checks
the single-flight guard, stale reply handling, monotone picker IDs, and
`CopyOnlyZeroPicker` across successful, warning, and error outcomes.

Shortcut recording pauses the active binding. A failed or cancelled recording
restores the paused binding when the backend can reacquire it. The committed
settings survive a registration or restoration failure. A rebind of a live
shortcut registers the replacement before retiring the old binding.

`BusyEventuallyClears` checks `[]<>(~Busy)` under weak fairness for completing
a copy and processing a matching picker reply. That fairness assumption means
an open picker eventually receives a choice or dismissal.

The Rust tests drive the copy and shortcut controllers with an in-memory
clipboard and simulated backend. The model-based test compares replies,
clipboard payloads and revisions, lifecycle events, notifications, and
configuration after each generated action. TLC covers native lock and
pasteboard interleavings as well as the lifecycle's fairness property.
