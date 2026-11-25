import CopiedFormal.Text

/-!
# Chromium Pickle custom data (`org.chromium.web-custom-data`)

Executable model of the encoder in `apps/macos/Pasteboard.swift` (`signalPasteboardData`)
and the Rust codec `copied_codec::pickle`.

Wire format (all integers little-endian `u32`):

```
payload_size | entry_count | (len mime | utf16le mime | pad | len value | utf16le value | pad)*
```

Bytes and UTF-16 code units are modelled as `List Nat`. Each string starts on a 4-byte
boundary, so its padding depends only on its own code-unit count: an odd count needs two
zero bytes, an even count needs none.
-/

namespace CopiedFormal.Pickle

/-- Little-endian bytes of a `u32`. -/
def u32Bytes (n : Nat) : List Nat :=
  [n % 256, n / 256 % 256, n / 65536 % 256, n / 16777216 % 256]

/-- UTF-16LE bytes of a code-unit list. -/
def unitBytes : List Nat → List Nat
  | [] => []
  | u :: us => u % 256 :: u / 256 % 256 :: unitBytes us

/-- Zero padding that restores 4-byte alignment after `n` UTF-16 code units. -/
def padLen (n : Nat) : Nat := if n % 2 = 0 then 0 else 2

/-- One length-prefixed, padded UTF-16 string. -/
def encodeStr (s : List Nat) : List Nat :=
  u32Bytes s.length ++ unitBytes s ++ List.replicate (padLen s.length) 0

/-- Entries are `(mime, value)` pairs of UTF-16 code-unit lists. -/
abbrev Entry := List Nat × List Nat

def encodeEntries : List Entry → List Nat
  | [] => []
  | (m, v) :: es => encodeStr m ++ encodeStr v ++ encodeEntries es

/-- Bytes after the 4-byte size header. -/
def payload (es : List Entry) : List Nat :=
  u32Bytes es.length ++ encodeEntries es

/-- Full Pickle: payload size header followed by the payload. -/
def encode (es : List Entry) : List Nat :=
  u32Bytes (payload es).length ++ payload es

/-! ## Decoder (strict) -/

def readU32 : List Nat → Option (Nat × List Nat)
  | a :: b :: c :: d :: rest =>
      if a < 256 ∧ b < 256 ∧ c < 256 ∧ d < 256 then
        some (a + 256 * b + 65536 * c + 16777216 * d, rest)
      else none
  | _ => none

def readUnits : Nat → List Nat → Option (List Nat × List Nat)
  | 0, bs => some ([], bs)
  | k + 1, a :: b :: rest =>
      if a < 256 ∧ b < 256 then
        match readUnits k rest with
        | some (us, r) => some ((a + 256 * b) :: us, r)
        | none => none
      else none
  | _ + 1, _ => none

/-- Consumes `k` padding bytes, all of which must be zero. -/
def readPad : Nat → List Nat → Option (List Nat)
  | 0, bs => some bs
  | k + 1, 0 :: rest => readPad k rest
  | _ + 1, _ => none

def readStr (bs : List Nat) : Option (List Nat × List Nat) :=
  match readU32 bs with
  | none => none
  | some (n, r) =>
    match readUnits n r with
    | none => none
    | some (us, r) =>
      match readPad (padLen n) r with
      | none => none
      | some r => some (us, r)

def readEntries : Nat → List Nat → Option (List Entry × List Nat)
  | 0, bs => some ([], bs)
  | k + 1, bs =>
    match readStr bs with
    | none => none
    | some (m, r) =>
      match readStr r with
      | none => none
      | some (v, r) =>
        match readEntries k r with
        | none => none
        | some (es, r) => some ((m, v) :: es, r)

/-- Strict decoder: exact payload size, 4-byte alignment, zero padding, full consumption. -/
def decode (bs : List Nat) : Option (List Entry) :=
  match readU32 bs with
  | none => none
  | some (size, r) =>
    if r.length = size ∧ size % 4 = 0 then
      match readU32 r with
      | none => none
      | some (count, r) =>
        match readEntries count r with
        | some (es, []) => some es
        | _ => none
    else none

/-! ## Well-formedness -/

def UnitsOk (s : List Nat) : Prop := (∀ u ∈ s, u < 65536) ∧ s.length < 4294967296

/-- Inputs the encoder accepts: real UTF-16 code units and every count fits in a `u32`.
The Rust encoder returns `CodecError::Overflow` outside this set. -/
def WellFormed (es : List Entry) : Prop :=
  (∀ e ∈ es, UnitsOk e.1 ∧ UnitsOk e.2) ∧ es.length < 4294967296 ∧
    (payload es).length < 4294967296

/-! ## Lemmas -/


theorem u32Bytes_length (n : Nat) : (u32Bytes n).length = 4 := rfl

theorem unitBytes_length (s : List Nat) : (unitBytes s).length = 2 * s.length := by
  induction s with
  | nil => rfl
  | cons u us ih => simp [unitBytes, ih]; omega

theorem encodeStr_length (s : List Nat) :
    (encodeStr s).length = 4 + 2 * s.length + padLen s.length := by
  simp [encodeStr, u32Bytes_length, unitBytes_length]; omega

theorem encodeStr_length_mod (s : List Nat) : (encodeStr s).length % 4 = 0 := by
  rw [encodeStr_length]; unfold padLen; split <;> omega

theorem encodeEntries_length_mod (es : List Entry) : (encodeEntries es).length % 4 = 0 := by
  induction es with
  | nil => rfl
  | cons e es ih =>
    obtain ⟨m, v⟩ := e
    have hm := encodeStr_length_mod m
    have hv := encodeStr_length_mod v
    simp only [encodeEntries, List.length_append]
    omega

theorem readU32_u32Bytes (n : Nat) (rest : List Nat) (h : n < 4294967296) :
    readU32 (u32Bytes n ++ rest) = some (n, rest) := by
  simp only [u32Bytes, List.cons_append, List.nil_append, readU32]
  rw [ite_pos (by omega)]
  congr 2
  omega

theorem readUnits_unitBytes (s rest : List Nat) (h : ∀ u ∈ s, u < 65536) :
    readUnits s.length (unitBytes s ++ rest) = some (s, rest) := by
  induction s with
  | nil => rfl
  | cons u us ih =>
    have hu : u < 65536 := h u (List.mem_cons_self ..)
    have hus : ∀ x ∈ us, x < 65536 := fun x hx => h x (List.mem_cons_of_mem _ hx)
    simp only [List.length_cons, unitBytes, List.cons_append, readUnits]
    rw [ite_pos (by omega), ih hus]
    simp only [Option.some.injEq, Prod.mk.injEq, List.cons.injEq, and_true]
    omega

theorem readPad_replicate (k : Nat) (rest : List Nat) :
    readPad k (List.replicate k 0 ++ rest) = some rest := by
  induction k with
  | zero => rfl
  | succ k ih => simp [List.replicate_succ, readPad, ih]

theorem readStr_encodeStr (s rest : List Nat) (h : UnitsOk s) :
    readStr (encodeStr s ++ rest) = some (s, rest) := by
  unfold readStr encodeStr
  simp only [List.append_assoc]
  rw [readU32_u32Bytes _ _ h.2]
  dsimp only
  rw [readUnits_unitBytes _ _ h.1]
  dsimp only
  rw [readPad_replicate]

theorem readEntries_encodeEntries (es : List Entry) (rest : List Nat)
    (h : ∀ e ∈ es, UnitsOk e.1 ∧ UnitsOk e.2) :
    readEntries es.length (encodeEntries es ++ rest) = some (es, rest) := by
  induction es with
  | nil => rfl
  | cons e es ih =>
    obtain ⟨m, v⟩ := e
    have he := h (m, v) (List.mem_cons_self ..)
    have hes : ∀ x ∈ es, UnitsOk x.1 ∧ UnitsOk x.2 :=
      fun x hx => h x (List.mem_cons_of_mem _ hx)
    simp only [List.length_cons, encodeEntries, List.append_assoc, readEntries]
    rw [readStr_encodeStr _ _ he.1]
    dsimp only
    rw [readStr_encodeStr _ _ he.2]
    dsimp only
    rw [ih hes]

/-! ## Theorems -/

/-- Decoding an encoded entry list returns exactly the original entries. -/
theorem decode_encode_roundtrip (es : List Entry) (h : WellFormed es) :
    decode (encode es) = some es := by
  obtain ⟨hes, hcount, hsize⟩ := h
  have hmod : (payload es).length % 4 = 0 := by
    have := encodeEntries_length_mod es
    simp only [payload, List.length_append, u32Bytes_length]
    omega
  unfold decode encode
  rw [readU32_u32Bytes _ _ hsize]
  dsimp only
  rw [ite_pos ⟨rfl, hmod⟩]
  unfold payload
  rw [readU32_u32Bytes _ _ hcount]
  dsimp only
  have := readEntries_encodeEntries es [] hes
  rw [List.append_nil] at this
  rw [this]

/-- Every encoded Pickle (header plus payload) is a whole number of 4-byte words. -/
theorem encode_aligned (es : List Entry) : (encode es).length % 4 = 0 := by
  have := encodeEntries_length_mod es
  simp only [encode, payload, List.length_append, u32Bytes_length]
  omega

/-- The 4-byte header declares exactly the number of bytes that follow it. -/
theorem encode_header_length (es : List Entry) (h : (payload es).length < 4294967296) :
    (encode es).length = 4 + (payload es).length ∧
      readU32 (encode es) = some ((encode es).length - 4, payload es) := by
  have hl : (encode es).length = 4 + (payload es).length := by
    simp [encode, u32Bytes_length]
  refine ⟨hl, ?_⟩
  rw [hl, Nat.add_sub_cancel_left]
  exact readU32_u32Bytes _ _ h

/-! ## Executable helpers for test vectors -/

def entriesOf (es : List (String × String)) : List Entry :=
  es.map fun (m, v) => (Text.utf16 m, Text.utf16 v)

/-- Signal Desktop payload: a single `text/signal` entry. -/
def encodeSignal (html : String) : List Nat := encode (entriesOf [("text/signal", html)])

end CopiedFormal.Pickle
