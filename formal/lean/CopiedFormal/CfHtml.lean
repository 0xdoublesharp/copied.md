import CopiedFormal.Text

/-!
# Windows `CF_HTML` envelope

Executable model of `copied_codec::cf_html::encode_cf_html`. Bytes are UTF-8 code units as
`List Nat`. The envelope is:

```
Version:0.9\r\n
StartHTML:XXXXXXXXXX\r\n
EndHTML:XXXXXXXXXX\r\n
StartFragment:XXXXXXXXXX\r\n
EndFragment:XXXXXXXXXX\r\n
<html><body>\r\n
<!--StartFragment-->{fragment}<!--EndFragment-->\r\n
</body></html>
```

Each offset is a 10-digit, zero-padded decimal byte offset, so the header is always
105 bytes long whatever the fragment is.
-/

namespace CopiedFormal.CfHtml

/-- `"Version:0.9\r\n"` (13 bytes) -/
def versionLine : List Nat := [86, 101, 114, 115, 105, 111, 110, 58, 48, 46, 57, 13, 10]
/-- `"StartHTML:"` (10 bytes) -/
def keyStartHtml : List Nat := [83, 116, 97, 114, 116, 72, 84, 77, 76, 58]
/-- `"EndHTML:"` (8 bytes) -/
def keyEndHtml : List Nat := [69, 110, 100, 72, 84, 77, 76, 58]
/-- `"StartFragment:"` (14 bytes) -/
def keyStartFragment : List Nat :=
  [83, 116, 97, 114, 116, 70, 114, 97, 103, 109, 101, 110, 116, 58]
/-- `"EndFragment:"` (12 bytes) -/
def keyEndFragment : List Nat := [69, 110, 100, 70, 114, 97, 103, 109, 101, 110, 116, 58]
/-- `"\r\n"` (2 bytes) -/
def crlf : List Nat := [13, 10]
/-- `"<html><body>\r\n"` (14 bytes) -/
def htmlOpen : List Nat := [60, 104, 116, 109, 108, 62, 60, 98, 111, 100, 121, 62, 13, 10]
/-- `"<!--StartFragment-->"` (20 bytes) -/
def startMarker : List Nat :=
  [60, 33, 45, 45, 83, 116, 97, 114, 116, 70, 114, 97, 103, 109, 101, 110, 116, 45, 45, 62]
/-- `"<!--EndFragment-->"` (18 bytes) -/
def endMarker : List Nat :=
  [60, 33, 45, 45, 69, 110, 100, 70, 114, 97, 103, 109, 101, 110, 116, 45, 45, 62]
/-- `"\r\n</body></html>"` (16 bytes) -/
def htmlClose : List Nat := [13, 10, 60, 47, 98, 111, 100, 121, 62, 60, 47, 104, 116, 109, 108, 62]

/-- Fixed-width, zero-padded, 10-digit ASCII decimal. -/
def digits10 (n : Nat) : List Nat :=
  [48 + n / 1000000000 % 10, 48 + n / 100000000 % 10, 48 + n / 10000000 % 10,
   48 + n / 1000000 % 10, 48 + n / 100000 % 10, 48 + n / 10000 % 10,
   48 + n / 1000 % 10, 48 + n / 100 % 10, 48 + n / 10 % 10, 48 + n % 10]

/-- Parses exactly ten ASCII digits. -/
def parse10 : List Nat → Option Nat
  | [a, b, c, d, e, f, g, h, i, j] =>
    let ds := [a, b, c, d, e, f, g, h, i, j]
    if ds.all (fun x => 48 ≤ x ∧ x ≤ 57) then
      some (ds.foldl (fun acc x => acc * 10 + (x - 48)) 0)
    else none
  | _ => none

/-- Byte length of everything before `<html>`. -/
def headerLen : Nat := 105

structure Offsets where
  startHtml : Nat
  endHtml : Nat
  startFragment : Nat
  endFragment : Nat
  deriving Repr, DecidableEq

/-- Offsets for a fragment of `n` UTF-8 bytes. -/
def offsets (n : Nat) : Offsets :=
  { startHtml := headerLen
    endHtml := headerLen + htmlOpen.length + startMarker.length + n + endMarker.length +
      htmlClose.length
    startFragment := headerLen + htmlOpen.length + startMarker.length
    endFragment := headerLen + htmlOpen.length + startMarker.length + n }

def header (o : Offsets) : List Nat :=
  versionLine ++
  keyStartHtml ++ digits10 o.startHtml ++ crlf ++
  keyEndHtml ++ digits10 o.endHtml ++ crlf ++
  keyStartFragment ++ digits10 o.startFragment ++ crlf ++
  keyEndFragment ++ digits10 o.endFragment ++ crlf

/-- The full `CF_HTML` payload for a UTF-8 fragment. -/
def encode (frag : List Nat) : List Nat :=
  header (offsets frag.length) ++ htmlOpen ++ startMarker ++ frag ++ endMarker ++ htmlClose

/-! ## Theorems -/

theorem digits10_length (n : Nat) : (digits10 n).length = 10 := rfl

/-- The header is 105 bytes for every set of offsets: 10-digit fields never change width. -/
theorem header_length (o : Offsets) : (header o).length = headerLen := by
  simp [header, digits10_length, versionLine, keyStartHtml, keyEndHtml, keyStartFragment,
    keyEndFragment, crlf, headerLen]

/-- Each fixed-width field reads back as the offset written, for offsets below `10^10`. -/
theorem parse10_digits10 (n : Nat) (h : n < 10000000000) : parse10 (digits10 n) = some n := by
  simp only [digits10, parse10, List.all_cons, List.all_nil, List.foldl_cons, List.foldl_nil]
  simp only [Nat.le_add_right, Bool.and_true]
  have h1 : ∀ k, 48 + k % 10 ≤ 57 := fun k => by omega
  simp only [h1, Nat.add_sub_cancel_left, and_self, decide_true, Bool.and_self, ↓reduceIte,
    Option.some.injEq]
  omega

/-- The offsets are strictly ordered around the fragment. -/
theorem offsets_ordered (n : Nat) :
    (offsets n).startHtml < (offsets n).startFragment ∧
      (offsets n).startFragment ≤ (offsets n).endFragment ∧
      (offsets n).endFragment < (offsets n).endHtml := by
  simp [offsets, htmlOpen, startMarker, endMarker, htmlClose, headerLen]
  omega

/-- `EndHTML` is the total payload length. -/
theorem endHtml_eq_length (frag : List Nat) :
    (offsets frag.length).endHtml = (encode frag).length := by
  simp [encode, offsets, header_length]
  omega

theorem encode_split (frag : List Nat) :
    encode frag =
      (header (offsets frag.length) ++ htmlOpen ++ startMarker) ++ (frag ++ (endMarker ++ htmlClose)) := by
  simp [encode]

theorem prefix_length (frag : List Nat) :
    (header (offsets frag.length) ++ htmlOpen ++ startMarker).length =
      (offsets frag.length).startFragment := by
  simp [header_length, offsets]
  omega

/-- Slicing `[StartFragment, EndFragment)` recovers the fragment bytes exactly. -/
theorem fragment_slice (frag : List Nat) :
    ((encode frag).drop (offsets frag.length).startFragment).take
        ((offsets frag.length).endFragment - (offsets frag.length).startFragment) = frag := by
  rw [encode_split, List.drop_left' (prefix_length frag)]
  have : (offsets frag.length).endFragment - (offsets frag.length).startFragment =
      frag.length := by simp [offsets]
  rw [this, List.take_left]

/-- The fragment is immediately preceded by `<!--StartFragment-->`. -/
theorem start_marker_before (frag : List Nat) :
    ((encode frag).drop ((offsets frag.length).startFragment - startMarker.length)).take
        startMarker.length = startMarker := by
  have : encode frag =
      (header (offsets frag.length) ++ htmlOpen) ++ (startMarker ++ (frag ++ endMarker ++ htmlClose)) := by
    simp [encode]
  rw [this, List.drop_left' (by simp [header_length, offsets]), List.take_left]

/-- The fragment is immediately followed by `<!--EndFragment-->`. -/
theorem end_marker_after (frag : List Nat) :
    ((encode frag).drop (offsets frag.length).endFragment).take endMarker.length = endMarker := by
  have : encode frag =
      (header (offsets frag.length) ++ htmlOpen ++ startMarker ++ frag) ++ (endMarker ++ htmlClose) := by
    simp [encode]
  rw [this, List.drop_left' (by simp [header_length, offsets]; omega), List.take_left]

/-- `StartHTML` points at `<html>`. -/
theorem start_html_points_at_html (frag : List Nat) :
    (encode frag).drop (offsets frag.length).startHtml =
      htmlOpen ++ startMarker ++ frag ++ endMarker ++ htmlClose := by
  have : encode frag =
      header (offsets frag.length) ++ (htmlOpen ++ startMarker ++ frag ++ endMarker ++ htmlClose) := by
    simp [encode]
  rw [this, List.drop_left' (by rw [header_length]; rfl)]

/-- Each header field of an encoded envelope parses back to its offset (fragments under ~10 GB). -/
theorem header_fields_roundtrip (frag : List Nat) (h : frag.length < 9000000000) :
    parse10 (digits10 (offsets frag.length).startHtml) = some (offsets frag.length).startHtml ∧
    parse10 (digits10 (offsets frag.length).endHtml) = some (offsets frag.length).endHtml ∧
    parse10 (digits10 (offsets frag.length).startFragment) =
      some (offsets frag.length).startFragment ∧
    parse10 (digits10 (offsets frag.length).endFragment) =
      some (offsets frag.length).endFragment := by
  have hb : ∀ k, k ≤ 173 + frag.length → parse10 (digits10 k) = some k :=
    fun k hk => parse10_digits10 k (by omega)
  simp only [offsets, headerLen, htmlOpen, startMarker, endMarker, htmlClose, List.length_cons,
    List.length_nil] at *
  refine ⟨hb _ (by omega), hb _ (by omega), hb _ (by omega), hb _ (by omega)⟩

/-! ## Executable helpers for test vectors -/

def encodeString (frag : String) : List Nat := encode (Text.utf8 frag)

end CopiedFormal.CfHtml
