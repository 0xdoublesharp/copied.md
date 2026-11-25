import CopiedFormal.Text

/-!
# Signal spoiler plain-text fallback

Executable model of `signalSpoilerFallback` in `packages/converters/src/signal-spoilers.ts`
and of how `conversion.ts` uses it.

The HTML is modelled as a token stream of text runs and the literal `span`, `strong`, `em`
and `s` tags the TypeScript regex matches. `openTag .span true` is the literal
`<span data-spoiler="true">`.

The scanner keeps the same state as the TypeScript: a tag stack, the spoiler span depth and
a `malformed` flag. Spoiler interiors are defined separately by `depthBefore`, a fold over
span tags only that knows nothing about the scanner's stack or output.
-/

namespace CopiedFormal.Spoiler

inductive Tag where
  | span
  | strong
  | em
  | s
  deriving DecidableEq, Repr

inductive Tok where
  | text (s : String)
  | openTag (t : Tag) (spoiler : Bool)
  | closeTag (t : Tag)
  deriving DecidableEq, Repr

/-- Plain-text fallback pieces. `text i s` is the input text token at position `i`. -/
inductive Out where
  | text (idx : Nat) (s : String)
  | placeholder
  deriving DecidableEq, Repr

structure State where
  stack : List Tag
  depth : Nat
  malformed : Bool
  out : List Out

def State.init : State := ⟨[], 0, false, []⟩

def step (idx : Nat) (st : State) : Tok → State
  | .text s => if st.depth = 0 then { st with out := st.out ++ [.text idx s] } else st
  | .openTag .span sp =>
    if st.depth = 0 then
      if sp then ⟨.span :: st.stack, 1, st.malformed, st.out ++ [.placeholder]⟩
      else ⟨.span :: st.stack, 0, st.malformed, st.out⟩
    else ⟨.span :: st.stack, st.depth + 1, st.malformed, st.out⟩
  | .openTag t _ => { st with stack := t :: st.stack }
  | .closeTag t =>
    -- A close tag pops the stack. Inside a spoiler, a mismatched or missing opener is malformed.
    ⟨st.stack.tail, (if t = .span then st.depth - 1 else st.depth),
     st.malformed || (st.depth != 0 && st.stack.head? != some t), st.out⟩

def scanGo : Nat → State → List Tok → State
  | _, st, [] => st
  | idx, st, t :: ts => scanGo (idx + 1) (step idx st t) ts

structure Result where
  plain : List Out
  malformed : Bool
  deriving Repr

def scan (toks : List Tok) : Result :=
  let st := scanGo 0 State.init toks
  { plain := st.out, malformed := st.malformed || decide (st.depth > 0) }

def hasSpoiler (toks : List Tok) : Bool := toks.contains (.openTag .span true)

/-- The clipboard representations `conversion.ts` builds for the Signal target. -/
structure Reps where
  plain : List Out
  /-- `text/signal` rich payload. `none` means it is withheld. -/
  signal : Option (List Tok)
  spoilerOmitted : Bool
  deriving Repr

def representations (toks : List Tok) : Reps :=
  let r := scan toks
  { plain := r.plain
    signal := if r.malformed then none else some toks
    spoilerOmitted := hasSpoiler toks }

/-! ## Independent definition of spoiler interiors -/

/-- Span nesting depth counted from the first spoiler span. Other tags are ignored. -/
def spanDepth (d : Nat) : Tok → Nat
  | .openTag .span true => d + 1
  | .openTag .span false => if d = 0 then 0 else d + 1
  | .closeTag .span => d - 1
  | _ => d

/-- Spoiler depth just before token `i`. Text token `i` is inside a spoiler iff this is
positive. -/
def depthBefore (toks : List Tok) (i : Nat) : Nat := (toks.take i).foldl spanDepth 0

def Interior (toks : List Tok) (i : Nat) : Prop :=
  (∃ s, toks[i]? = some (.text s)) ∧ depthBefore toks i > 0

/-! ## Lemmas -/

theorem step_depth (idx : Nat) (st : State) (t : Tok) :
    (step idx st t).depth = spanDepth st.depth t := by
  by_cases hd : st.depth = 0
  · cases t with
    | text s => simp [step, spanDepth, hd]
    | openTag tag sp => cases tag <;> cases sp <;> simp [step, spanDepth, hd]
    | closeTag tag => cases tag <;> simp [step, spanDepth, hd]
  · cases t with
    | text s => simp [step, spanDepth, hd]
    | openTag tag sp => cases tag <;> cases sp <;> simp [step, spanDepth, hd]
    | closeTag tag => cases tag <;> simp [step, spanDepth]

theorem step_out (idx : Nat) (st : State) (t : Tok) (i : Nat) (s : String)
    (h : Out.text i s ∈ (step idx st t).out) :
    Out.text i s ∈ st.out ∨ (i = idx ∧ t = .text s ∧ st.depth = 0) := by
  by_cases hd : st.depth = 0
  · cases t with
    | text s' =>
      simp only [step, hd, ↓reduceIte, List.mem_append, List.mem_singleton, Out.text.injEq] at h
      rcases h with h | ⟨rfl, rfl⟩
      · exact Or.inl h
      · exact Or.inr ⟨rfl, rfl, hd⟩
    | openTag tag sp =>
      left
      cases tag <;> cases sp <;> simpa [step, hd] using h
    | closeTag tag => left; simpa [step] using h
  · left
    cases t with
    | text s' => simpa [step, hd] using h
    | openTag tag sp => cases tag <;> cases sp <;> simpa [step, hd] using h
    | closeTag tag => simpa [step] using h

theorem scanGo_out (toks : List Tok) (idx : Nat) (st : State) (i : Nat) (s : String)
    (h : Out.text i s ∈ (scanGo idx st toks).out) :
    Out.text i s ∈ st.out ∨
      (idx ≤ i ∧ toks[i - idx]? = some (.text s) ∧
        (toks.take (i - idx)).foldl spanDepth st.depth = 0) := by
  induction toks generalizing idx st with
  | nil => exact Or.inl h
  | cons t ts ih =>
    simp only [scanGo] at h
    rcases ih (idx + 1) _ h with h1 | ⟨hle, hget, hdepth⟩
    · rcases step_out idx st t i s h1 with h2 | ⟨rfl, rfl, hd⟩
      · exact Or.inl h2
      · right; simp [hd]
    · right
      have hpos : i - idx = (i - (idx + 1)) + 1 := by omega
      refine ⟨by omega, ?_, ?_⟩
      · rw [hpos]; simpa using hget
      · rw [hpos, List.take_succ_cons, List.foldl_cons, ← step_depth idx st t]
        exact hdepth

/-! ## Theorems -/

/-- Every text the fallback shows is the input text at that position, and that position is
outside every spoiler. Holds whether or not the input is malformed. -/
theorem visible_text_outside_spoilers (toks : List Tok) (i : Nat) (s : String)
    (h : Out.text i s ∈ (scan toks).plain) :
    toks[i]? = some (.text s) ∧ ¬ Interior toks i := by
  simp only [scan] at h
  rcases scanGo_out toks 0 State.init i s h with h0 | ⟨_, hget, hdepth⟩
  · simp [State.init] at h0
  · simp only [Nat.sub_zero, State.init] at hget hdepth
    refine ⟨hget, fun ⟨_, hpos⟩ => ?_⟩
    unfold depthBefore at hpos
    omega

/-- Spoiler noninterference: when the input is not flagged malformed, no text token from
inside a spoiler appears in the plain-text fallback. -/
theorem spoiler_noninterference (toks : List Tok) (_h : (scan toks).malformed = false) :
    ∀ i s, Interior toks i → Out.text i s ∉ (scan toks).plain := by
  intro i s hint hmem
  exact (visible_text_outside_spoilers toks i s hmem).2 hint

/-- Malformed spoiler markup fails closed: the rich `text/signal` payload is withheld. -/
theorem malformed_fails_closed (toks : List Tok) (h : (scan toks).malformed = true) :
    (representations toks).signal = none := by
  simp [representations, h]

/-- Well-formed input keeps the rich payload unchanged. -/
theorem wellformed_keeps_rich (toks : List Tok) (h : (scan toks).malformed = false) :
    (representations toks).signal = some toks := by
  simp [representations, h]

/-! ## Executable helpers for test vectors -/

def placeholderText : String := "[spoiler omitted]"

def renderPlain (outs : List Out) : String :=
  String.join (outs.map fun
    | .text _ s => s
    | .placeholder => placeholderText)

end CopiedFormal.Spoiler
