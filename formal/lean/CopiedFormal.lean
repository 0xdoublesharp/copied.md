import CopiedFormal.Text
import CopiedFormal.Pickle
import CopiedFormal.CfHtml
import CopiedFormal.Rtf
import CopiedFormal.Spoiler
import CopiedFormal.Visibility

/-!
# copied.md formal models

Executable Lean 4 models of the clipboard codecs and selection logic, with proofs:

* `CopiedFormal.Pickle`: Chromium Pickle custom data round trip, alignment and header length.
* `CopiedFormal.CfHtml`: Windows `CF_HTML` offsets, fragment slice recovery, fixed-width header.
* `CopiedFormal.Rtf`: RTF escaping is ASCII and round-trips; styled runs have balanced groups.
* `CopiedFormal.Spoiler`: Signal spoiler fallback noninterference and fail-closed behavior.
* `CopiedFormal.Visibility`: format visibility, preferred target and per-app pins.

`lake exe vectors` prints JSON test vectors computed by these same functions. The Rust
crates check against those vectors.
-/

namespace CopiedFormal

/-- Version of the `vectors.json` schema. -/
def schemaVersion : Nat := 1

theorem schemaVersion_eq_one : schemaVersion = 1 := rfl

end CopiedFormal
