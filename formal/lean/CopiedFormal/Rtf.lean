import CopiedFormal.Text

/-!
# RTF text escaping and styled-run emission

Executable model of `escapeRtf` in `packages/converters/src/render-text.ts` and the Rust codec
`copied_codec::rtf`. Input is a list of UTF-16 code units. Output is ASCII codes.

* `\`, `{` and `}` become `\\`, `\{` and `\}`.
* Tab, newline and printable ASCII `0x20..=0x7E` pass through.
* Any other code unit `u` becomes `\uN?`, where `N` is `u` read as a signed 16-bit integer.

`unescape` accepts exactly the following, and rejects everything else:

| input            | meaning                                   |
|------------------|-------------------------------------------|
| `\\`, `\{`, `\}` | the literal character                     |
| `\uN?`           | code unit `N`, `0 ≤ N ≤ 32767`            |
| `\u-N?`          | code unit `65536 - N`, `1 ≤ N ≤ 32768`    |
| unescaped `{`/`}`| rejected                                  |
| any other unit   | itself                                    |

`N` is one or more ASCII digits. Leading zeros are accepted. `\u-0?` is rejected.
-/

namespace CopiedFormal.Rtf

/-! ## Decimal digits -/

/-- Little-endian decimal digits (least significant first). -/
def digitsRev (n : Nat) : List Nat :=
  if n < 10 then [n] else n % 10 :: digitsRev (n / 10)
termination_by n
decreasing_by omega

/-- Big-endian ASCII decimal digits. -/
def decimal (n : Nat) : List Nat := (digitsRev n).reverse.map (· + 48)

/-! ## Escaper -/

def escapeUnit (u : Nat) : List Nat :=
  if u = 92 ∨ u = 123 ∨ u = 125 then [92, u]
  else if u = 9 ∨ u = 10 ∨ (32 ≤ u ∧ u ≤ 126) then [u]
  else if u < 32768 then 92 :: 117 :: (decimal u ++ [63])
  else 92 :: 117 :: 45 :: (decimal (65536 - u) ++ [63])

def escape : List Nat → List Nat
  | [] => []
  | u :: us => escapeUnit u ++ escape us

/-! ## Unescaper (one character per step) -/

inductive St where
  | text
  | slash
  /-- Inside `\u`: sign seen, at least one digit seen, accumulated magnitude. -/
  | num (neg : Bool) (seen : Bool) (acc : Nat)

def finish (neg : Bool) (acc : Nat) : Option Nat :=
  if neg then (if 1 ≤ acc ∧ acc ≤ 32768 then some (65536 - acc) else none)
  else if acc ≤ 32767 then some acc else none

def go : St → List Nat → Option (List Nat)
  | .text, [] => some []
  | .text, c :: rest =>
    if c = 92 then go .slash rest
    else if c = 123 ∨ c = 125 then none
    else (go .text rest).map (c :: ·)
  | .slash, [] => none
  | .slash, c :: rest =>
    if c = 92 ∨ c = 123 ∨ c = 125 then (go .text rest).map (c :: ·)
    else if c = 117 then go (.num false false 0) rest
    else none
  | .num _ _ _, [] => none
  | .num neg seen acc, c :: rest =>
    if 48 ≤ c ∧ c ≤ 57 then go (.num neg true (acc * 10 + (c - 48))) rest
    else if c = 45 ∧ neg = false ∧ seen = false then go (.num true false 0) rest
    else if c = 63 ∧ seen = true then
      match finish neg acc with
      | some u => (go .text rest).map (u :: ·)
      | none => none
    else none

/-- Strict RTF text-run unescaper returning UTF-16 code units. -/
def unescape (s : List Nat) : Option (List Nat) := go .text s

/-! ## Styled-run tree -/

inductive Ctrl where
  | bold
  | italic
  | strike
  deriving Repr

/-- `\b`, `\i`, `\strike`. -/
def Ctrl.bytes : Ctrl → List Nat
  | .bold => [92, 98]
  | .italic => [92, 105]
  | .strike => [92, 115, 116, 114, 105, 107, 101]

def ctrlBytes : List Ctrl → List Nat
  | [] => []
  | c :: cs => c.bytes ++ ctrlBytes cs

/-- Styled runs as emitted by the Telegram RTF renderer. -/
inductive Run where
  | text (units : List Nat)
  | group (ctrls : List Ctrl) (body : Run)
  | seq (a b : Run)
  deriving Repr

/-- `{\b\i body}` for a group with controls; `{body}` for a bare group. -/
def emit : Run → List Nat
  | .text us => escape us
  | .group cs body =>
    123 :: (ctrlBytes cs ++ (if cs.isEmpty then [] else [32]) ++ emit body ++ [125])
  | .seq a b => emit a ++ emit b

/-- Escape-aware RTF group depth scanner. `\x` consumes `x` without counting it.
Depth is a `Nat`. A `}` at depth 0 returns `none`, so a `some` result means depth never
went negative. -/
def braceScan : Nat → Bool → List Nat → Option Nat
  | d, _, [] => some d
  | d, true, _ :: rest => braceScan d false rest
  | d, false, c :: rest =>
    if c = 92 then braceScan d true rest
    else if c = 123 then braceScan (d + 1) false rest
    else if c = 125 then
      match d with
      | 0 => none
      | d + 1 => braceScan d false rest
    else braceScan d false rest

/-! ## Lemmas: digits -/

theorem digitsRev_lt (n : Nat) : ∀ d ∈ digitsRev n, d < 10 := by
  fun_induction digitsRev n with
  | case1 n h => simp; omega
  | case2 n h ih =>
    intro d hd
    simp only [List.mem_cons] at hd
    rcases hd with hd | hd
    · omega
    · exact ih d hd

theorem digitsRev_ne_nil (n : Nat) : digitsRev n ≠ [] := by
  rw [digitsRev]; split <;> simp

theorem digitsRev_value (n : Nat) :
    (digitsRev n).foldr (fun d a => a * 10 + d) 0 = n := by
  fun_induction digitsRev n with
  | case1 n h => simp
  | case2 n h ih => simp only [List.foldr_cons, ih]; omega

theorem decimal_mem (n c : Nat) (h : c ∈ decimal n) : 48 ≤ c ∧ c ≤ 57 := by
  simp only [decimal, List.mem_map, List.mem_reverse] at h
  obtain ⟨d, hd, rfl⟩ := h
  have := digitsRev_lt n d hd
  omega

/-! ## Lemmas: unescape ∘ escape -/

theorem go_num_digits (ds rest : List Nat) (neg seen : Bool) (acc : Nat)
    (h : ∀ d ∈ ds, d < 10) :
    go (.num neg seen acc) (ds.map (· + 48) ++ rest) =
      go (.num neg (seen || !ds.isEmpty) (ds.foldl (fun a d => a * 10 + d) acc)) rest := by
  induction ds generalizing seen acc with
  | nil => simp
  | cons d ds ih =>
    have hd : d < 10 := h d (List.mem_cons_self ..)
    have hds : ∀ x ∈ ds, x < 10 := fun x hx => h x (List.mem_cons_of_mem _ hx)
    simp only [List.map_cons, List.cons_append, go]
    rw [ite_pos (by omega), ih true _ hds]
    simp only [Nat.add_sub_cancel, Bool.true_or, List.isEmpty_cons, Bool.not_false,
      Bool.or_true, List.foldl_cons]

theorem go_decimal (n : Nat) (rest : List Nat) (neg : Bool) :
    go (.num neg false 0) (decimal n ++ rest) = go (.num neg true n) rest := by
  unfold decimal
  rw [go_num_digits _ _ _ _ _ (by
    intro d hd; exact digitsRev_lt n d (List.mem_reverse.mp hd))]
  have hne : (digitsRev n).reverse.isEmpty = false := by
    simp [digitsRev_ne_nil]
  rw [hne, List.foldl_reverse, digitsRev_value]
  rfl

theorem go_escapeUnit (u : Nat) (rest : List Nat) (hu : u < 65536) :
    go .text (escapeUnit u ++ rest) = (go .text rest).map (u :: ·) := by
  unfold escapeUnit
  split
  · next h =>
    simp only [List.cons_append, List.nil_append, go]
    rw [ite_pos (by decide), ite_pos h]
  · next h1 =>
    split
    · next h2 =>
      simp only [List.cons_append, List.nil_append, go]
      rw [ite_neg (by omega), ite_neg (by omega)]
    · next h2 =>
      split
      · next h3 =>
        simp only [List.cons_append, List.append_assoc, go]
        rw [ite_pos (by decide), ite_neg (by decide), ite_pos (by decide), go_decimal]
        simp only [List.nil_append, go]
        rw [ite_neg (by decide), ite_neg (by decide), ite_pos (by simp)]
        simp only [finish, Bool.false_eq_true, ↓reduceIte]
        rw [ite_pos (by omega)]
      · next h3 =>
        simp only [List.cons_append, List.append_assoc, go]
        rw [ite_pos (by decide), ite_neg (by decide), ite_pos (by decide)]
        rw [ite_neg (by decide), ite_pos (by simp), go_decimal]
        simp only [List.nil_append, go]
        rw [ite_neg (by decide), ite_neg (by decide), ite_pos (by simp)]
        simp only [finish, ↓reduceIte]
        rw [ite_pos (by omega)]
        have : 65536 - (65536 - u) = u := by omega
        rw [this]

/-! ## Lemmas: brace scanning -/

def Plain (l : List Nat) : Prop := ∀ c ∈ l, c ≠ 92 ∧ c ≠ 123 ∧ c ≠ 125

theorem braceScan_plain (l rest : List Nat) (d : Nat) (h : Plain l) :
    braceScan d false (l ++ rest) = braceScan d false rest := by
  induction l with
  | nil => rfl
  | cons c cs ih =>
    have hc := h c (List.mem_cons_self ..)
    have hcs : Plain cs := fun x hx => h x (List.mem_cons_of_mem _ hx)
    simp only [List.cons_append, braceScan]
    rw [ite_neg hc.1, ite_neg hc.2.1, ite_neg hc.2.2, ih hcs]

theorem decimal_plain (n : Nat) : Plain (decimal n ++ [63]) := by
  intro c hc
  simp only [List.mem_append, List.mem_singleton] at hc
  rcases hc with hc | rfl
  · have := decimal_mem n c hc; omega
  · decide

theorem braceScan_escapeUnit (u d : Nat) (rest : List Nat) :
    braceScan d false (escapeUnit u ++ rest) = braceScan d false rest := by
  unfold escapeUnit
  split
  · simp [braceScan]
  · next h1 =>
    split
    · simp only [List.cons_append, List.nil_append, braceScan]
      rw [ite_neg (by omega), ite_neg (by omega), ite_neg (by omega)]
    · split
      · simp only [List.cons_append, List.append_assoc, braceScan, ↓reduceIte]
        simpa using braceScan_plain _ rest d (decimal_plain u)
      · simp only [List.cons_append, List.append_assoc, braceScan, ↓reduceIte]
        rw [ite_neg (by decide), ite_neg (by decide), ite_neg (by decide)]
        simpa using braceScan_plain _ rest d (decimal_plain (65536 - u))

theorem braceScan_escape (us rest : List Nat) (d : Nat) :
    braceScan d false (escape us ++ rest) = braceScan d false rest := by
  induction us with
  | nil => rfl
  | cons u us ih => simp only [escape, List.append_assoc, braceScan_escapeUnit, ih]

theorem braceScan_ctrlBytes (cs : List Ctrl) (rest : List Nat) (d : Nat) :
    braceScan d false (ctrlBytes cs ++ rest) = braceScan d false rest := by
  induction cs with
  | nil => rfl
  | cons c cs ih =>
    simp only [ctrlBytes, List.append_assoc]
    cases c <;> simp [Ctrl.bytes, braceScan, ih]

theorem braceScan_emit (t : Run) (rest : List Nat) (d : Nat) :
    braceScan d false (emit t ++ rest) = braceScan d false rest := by
  induction t generalizing d rest with
  | text us => exact braceScan_escape us rest d
  | group cs body ih =>
    simp only [emit, List.cons_append, List.append_assoc, List.nil_append, braceScan,
      ↓reduceIte]
    rw [ite_neg (by decide), braceScan_ctrlBytes]
    have hsp : braceScan (d + 1) false
        ((if cs.isEmpty = true then [] else [32]) ++ (emit body ++ 125 :: rest)) =
        braceScan (d + 1) false (emit body ++ 125 :: rest) := by
      split
      · rfl
      · apply braceScan_plain; intro c hc; simp at hc; omega
    rw [hsp, ih]
    simp [braceScan]
  | seq a b iha ihb => simp only [emit, List.append_assoc, iha, ihb]

theorem braceScan_prefix (l : List Nat) (d : Nat) (e : Bool) (r k : Nat)
    (h : braceScan d e l = some r) : (braceScan d e (l.take k)).isSome := by
  induction l generalizing d e k with
  | nil => simp [braceScan]
  | cons c cs ih =>
    cases k with
    | zero => simp [braceScan]
    | succ k =>
      simp only [List.take_succ_cons]
      cases e with
      | true => simp only [braceScan] at h ⊢; exact ih _ _ _ h
      | false =>
        simp only [braceScan] at h ⊢
        split
        · rw [ite_pos (by assumption)] at h; exact ih _ _ _ h
        · rw [ite_neg (by assumption)] at h
          split
          · rw [ite_pos (by assumption)] at h; exact ih _ _ _ h
          · rw [ite_neg (by assumption)] at h
            split
            · rw [ite_pos (by assumption)] at h
              cases d with
              | zero => simp at h
              | succ d => exact ih _ _ _ h
            · rw [ite_neg (by assumption)] at h; exact ih _ _ _ h

/-! ## Theorems -/

/-- Escaped RTF is pure 7-bit ASCII. -/
theorem rtf_output_ascii (us : List Nat) : ∀ c ∈ escape us, c < 128 := by
  induction us with
  | nil => simp [escape]
  | cons u us ih =>
    intro c hc
    simp only [escape, List.mem_append] at hc
    rcases hc with hc | hc
    · unfold escapeUnit at hc
      split at hc
      · simp at hc; omega
      · split at hc
        · simp at hc; omega
        · split at hc
          · simp only [List.mem_cons, List.mem_append, List.not_mem_nil, or_false] at hc
            rcases hc with rfl | rfl | hc | rfl
            · decide
            · decide
            · have := decimal_mem _ _ hc; omega
            · decide
          · simp only [List.mem_cons, List.mem_append, List.not_mem_nil, or_false] at hc
            rcases hc with rfl | rfl | rfl | hc | rfl
            · decide
            · decide
            · decide
            · have := decimal_mem _ _ hc; omega
            · decide
    · exact ih c hc

/-- Unescaping escaped UTF-16 code units returns them exactly. -/
theorem unescape_escape_roundtrip (us : List Nat) (h : ∀ u ∈ us, u < 65536) :
    unescape (escape us) = some us := by
  unfold unescape
  induction us with
  | nil => rfl
  | cons u us ih =>
    have hu : u < 65536 := h u (List.mem_cons_self ..)
    have hus : ∀ x ∈ us, x < 65536 := fun x hx => h x (List.mem_cons_of_mem _ hx)
    simp only [escape]
    rw [go_escapeUnit _ _ hu, ih hus]
    rfl

/-- The styled-run emitter produces balanced RTF groups. Escape-aware brace depth never
goes negative on any prefix, and the full output ends at depth 0. -/
theorem styled_tree_balanced_braces (t : Run) :
    braceScan 0 false (emit t) = some 0 ∧
      ∀ k, (braceScan 0 false ((emit t).take k)).isSome := by
  have h : braceScan 0 false (emit t) = some 0 := by
    have := braceScan_emit t [] 0
    rw [List.append_nil] at this
    rw [this]; rfl
  exact ⟨h, fun k => braceScan_prefix _ _ _ _ k h⟩

/-! ## Executable helpers for test vectors -/

def escapeString (s : String) : String := Text.ofCodes (escape (Text.utf16 s))

def emitString (t : Run) : String := Text.ofCodes (emit t)

end CopiedFormal.Rtf
