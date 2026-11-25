/-!
# Text helpers shared by the executable models

Converts Lean `String`s to the `List Nat` code-unit views the proofs work over. Used only at
the edges, by `Vectors.lean`: the proven functions themselves take and return `List Nat`.
-/

namespace CopiedFormal

/-- `if` with a true condition (core's `if_pos` is deprecated in this toolchain). -/
theorem ite_pos {α : Sort _} {c : Prop} [Decidable c] (h : c) (a b : α) :
    (if c then a else b) = a := by simp [h]

/-- `if` with a false condition. -/
theorem ite_neg {α : Sort _} {c : Prop} [Decidable c] (h : ¬c) (a b : α) :
    (if c then a else b) = b := by simp [h]

namespace Text

/-- UTF-16 code units of a `String` (surrogate pairs for astral code points). -/
def utf16 (s : String) : List Nat :=
  s.toList.flatMap fun c =>
    let n := c.toNat
    if n < 65536 then [n]
    else
      let m := n - 65536
      [0xD800 + m / 1024, 0xDC00 + m % 1024]

/-- UTF-8 bytes of a `String`. -/
def utf8 (s : String) : List Nat := s.toUTF8.toList.map (·.toNat)

/-- Renders a list of ASCII (or BMP non-surrogate) code points as a `String`. -/
def ofCodes (l : List Nat) : String := String.ofList (l.map Char.ofNat)

/-- Decodes UTF-16 code units, rejecting unpaired surrogates. -/
def fromUtf16 : List Nat → Option String
  | [] => some ""
  | u :: rest =>
    if 0xD800 ≤ u ∧ u < 0xDC00 then
      match rest with
      | l :: rest' =>
        if 0xDC00 ≤ l ∧ l < 0xE000 then
          (fromUtf16 rest').map fun s =>
            String.singleton (Char.ofNat (0x10000 + (u - 0xD800) * 1024 + (l - 0xDC00))) ++ s
        else none
      | [] => none
    else if 0xDC00 ≤ u ∧ u < 0xE000 then none
    else if u < 65536 then (fromUtf16 rest).map fun s => String.singleton (Char.ofNat u) ++ s
    else none

end Text

end CopiedFormal
