/-!
# Format visibility, preferred target, and per-app pins

Executable model of:

* `visibleFormats` and `preferredFormat` in `apps/browser-extension/src/lib/copy-formats.ts`;
* `resolveTarget` in `packages/converters/src/conversion.ts`, including the `html` and `slack`
  aliases;
* the per-app pin resolution in `apps/macos/DesktopApp.swift`. A pinned format is used only
  if it is still visible; otherwise the preferred format is used.
-/

namespace CopiedFormal.Visibility

inductive Target where
  | richText | plainText | htmlSource | slackPaste | discord | telegram | signal | reddit
  | facebook | twitter | sms | slackApi | teams | word | whatsapp
  deriving DecidableEq, Repr

/-- Catalog order from `conversion.ts`. -/
def allTargets : List Target :=
  [.richText, .plainText, .htmlSource, .slackPaste, .discord, .telegram, .signal, .reddit,
   .facebook, .twitter, .sms, .slackApi, .teams, .word, .whatsapp]

def Target.id : Target → String
  | .richText => "rich-text" | .plainText => "plain-text" | .htmlSource => "html-source"
  | .slackPaste => "slack-paste" | .discord => "discord" | .telegram => "telegram"
  | .signal => "signal" | .reddit => "reddit" | .facebook => "facebook" | .twitter => "twitter"
  | .sms => "sms" | .slackApi => "slack-api" | .teams => "teams" | .word => "word"
  | .whatsapp => "whatsapp"

def Target.enabledByDefault : Target → Bool
  | .teams | .word | .whatsapp => false
  | _ => true

/-- `resolveTarget`: legacy aliases first, then exact catalog ids. -/
def resolveTarget (value : String) : Option Target :=
  if value = "html" then some .richText
  else if value = "slack" then some .slackPaste
  else allTargets.find? (fun t => t.id == value)

/-- User visibility overrides. `none` means use the target's default. -/
abbrev Visibility := Target → Option Bool

def isVisible (vis : Visibility) (t : Target) : Bool := (vis t).getD t.enabledByDefault

/-- Visible formats in catalog order. Falls back to Rich Text when everything is hidden. -/
def visibleFormats (vis : Visibility) : List Target :=
  match allTargets.filter (isVisible vis) with
  | [] => [.richText]
  | v => v

def firstVisible (vis : Visibility) : Target :=
  match visibleFormats vis with
  | t :: _ => t
  | [] => .richText

/-- `preferredFormat`: the first preference that resolves to a visible target, else the first
visible target. -/
def preferredFormat (vis : Visibility) : List String → Target
  | [] => firstVisible vis
  | p :: ps =>
    match resolveTarget p with
    | some t => if t ∈ visibleFormats vis then t else preferredFormat vis ps
    | none => preferredFormat vis ps

/-- Per-app pins: application id ↦ target id (compared exactly, as in Swift). -/
abbrev Pins := List (String × String)

def pinnedTarget (vis : Visibility) (pins : Pins) (app : Option String) : Option Target :=
  match app with
  | none => none
  | some a =>
    match pins.lookup a with
    | none => none
    | some pinId => (visibleFormats vis).find? (fun t => t.id == pinId)

/-- The format the shortcut copies: the visible pin, else the preferred format. -/
def resolve (vis : Visibility) (pins : Pins) (app : Option String) (prefs : List String) :
    Target :=
  match pinnedTarget vis pins app with
  | some t => t
  | none => preferredFormat vis prefs

/-! ## Theorems -/

/-- There is always at least one visible format. -/
theorem visible_nonempty (vis : Visibility) : visibleFormats vis ≠ [] := by
  unfold visibleFormats
  split <;> simp_all

theorem firstVisible_mem (vis : Visibility) : firstVisible vis ∈ visibleFormats vis := by
  unfold firstVisible
  have := visible_nonempty vis
  split
  · next h => rw [h]; exact List.mem_cons_self ..
  · next h => exact absurd h this

/-- The preferred format is always visible. -/
theorem preferred_mem_visible (vis : Visibility) (prefs : List String) :
    preferredFormat vis prefs ∈ visibleFormats vis := by
  induction prefs with
  | nil => exact firstVisible_mem vis
  | cons p ps ih =>
    simp only [preferredFormat]
    split
    · split
      · assumption
      · exact ih
    · exact ih

theorem pinnedTarget_mem (vis : Visibility) (pins : Pins) (app : Option String) (t : Target)
    (h : pinnedTarget vis pins app = some t) : t ∈ visibleFormats vis := by
  unfold pinnedTarget at h
  split at h
  · simp at h
  · split at h
    · simp at h
    · exact List.mem_of_find?_eq_some h

/-- The resolved format is always visible. -/
theorem resolve_mem_visible (vis : Visibility) (pins : Pins) (app : Option String)
    (prefs : List String) : resolve vis pins app prefs ∈ visibleFormats vis := by
  unfold resolve
  split
  · next t h => exact pinnedTarget_mem vis pins app t h
  · exact preferred_mem_visible vis prefs

/-- A pin that names a hidden format is never selected, and neither is any other hidden
format. -/
theorem hidden_pin_never_selected (vis : Visibility) (pins : Pins) (app : String)
    (prefs : List String) (t : Target) (_hpin : pins.lookup app = some t.id)
    (hidden : t ∉ visibleFormats vis) : resolve vis pins (some app) prefs ≠ t := by
  intro heq
  have := resolve_mem_visible vis pins (some app) prefs
  rw [heq] at this
  exact hidden this

/-- A visible pin wins over every preference. -/
theorem visible_pin_selected (vis : Visibility) (pins : Pins) (app : String)
    (prefs : List String) (t : Target) (hpin : pins.lookup app = some t.id)
    (hfirst : (visibleFormats vis).find? (fun x => x.id == t.id) = some t) :
    resolve vis pins (some app) prefs = t := by
  simp [resolve, pinnedTarget, hpin, hfirst]

/-! ## Executable helpers for test vectors -/

/-- Builds a visibility function from explicit `(target id, visible)` overrides. -/
def visibilityOf (overrides : List (String × Bool)) : Visibility :=
  fun t => overrides.lookup t.id

end CopiedFormal.Visibility
