import CopiedFormal

/-!
# `lake exe vectors`

Prints JSON test vectors computed by the proven functions in `CopiedFormal`. The Rust test
`crates/copied-codec/tests/lean_vectors.rs` reads the checked-in copy at
`formal/lean/vectors.json`. Regenerate it with:

```
lake exe vectors > vectors.json
```
-/

open CopiedFormal

namespace Json

def hexDigit (n : Nat) : Char := "0123456789abcdef".toList.getD n '0'

def escapeChar (c : Char) : String :=
  if c = '"' then "\\\""
  else if c = '\\' then "\\\\"
  else if c = '\n' then "\\n"
  else if c = '\r' then "\\r"
  else if c = '\t' then "\\t"
  else if c.toNat < 0x20 ∨ c.toNat = 0x7F then
    "\\u00" ++ String.singleton (hexDigit (c.toNat / 16)) ++
      String.singleton (hexDigit (c.toNat % 16))
  else String.singleton c

def str (s : String) : String := "\"" ++ String.join (s.toList.map escapeChar) ++ "\""

def nat (n : Nat) : String := toString n

def bool (b : Bool) : String := if b then "true" else "false"

def arr (xs : List String) : String := "[" ++ ", ".intercalate xs ++ "]"

def obj (fields : List (String × String)) : String :=
  "{" ++ ", ".intercalate (fields.map fun (k, v) => str k ++ ": " ++ v) ++ "}"

def opt (o : Option String) : String := o.getD "null"

def hex (bytes : List Nat) : String :=
  str (String.join (bytes.map fun b =>
    String.singleton (hexDigit (b / 16 % 16)) ++ String.singleton (hexDigit (b % 16))))

end Json

open Json

/-! ## Pickle -/

def pickleValid : List (String × List (String × String)) :=
  [ ("empty", []),
    ("signal-ascii", [("text/signal", "<b>hi</b>")]),
    ("signal-emoji", [("text/signal", "Hi 🚀 Café <i>naïve</i>")]),
    ("even-and-empty", [("text/plain", "even"), ("text/html", "")]),
    ("two-even", [("ab", "cdef")]),
    ("cjk", [("text/plain", "日本語テキスト")]) ]

def pickleValidJson : String :=
  arr (pickleValid.map fun (name, es) =>
    let bytes := Pickle.encode (Pickle.entriesOf es)
    let decoded := Pickle.decode bytes == some (Pickle.entriesOf es)
    obj [("name", str name),
         ("entries", arr (es.map fun (m, v) => arr [str m, str v])),
         ("hex", hex bytes),
         ("aligned", bool (bytes.length % 4 == 0)),
         ("lean_roundtrip", bool decoded)])

/-- Malformed or non-canonical buffers. `entries` is `null` when the strict decoder must reject. -/
def pickleDecode : List (String × List Nat) :=
  let good := Pickle.encode (Pickle.entriesOf [("text/signal", "<b>hi</b>")])
  [ ("valid", good),
    ("truncated-header", [4, 0]),
    ("size-mismatch", [8, 0, 0, 0, 0, 0, 0, 0]),
    ("size-unaligned", [2, 0, 0, 0, 0, 0]),
    ("empty-count-only", [4, 0, 0, 0, 0, 0, 0, 0]),
    ("missing-count", [0, 0, 0, 0]),
    ("count-too-large", [4, 0, 0, 0, 1, 0, 0, 0]),
    ("unit-truncated", [12, 0, 0, 0, 1, 0, 0, 0, 3, 0, 0, 0, 97, 0, 0, 0]),
    ("nonzero-padding", [20, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 97, 0, 7, 0,
                         1, 0, 0, 0, 98, 0, 0, 0]),
    ("trailing-bytes", [16, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    ("lone-surrogate", [16, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0x3D, 0xD8, 0, 0,
                        0, 0, 0, 0]),
    ("empty-strings", [12, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]) ]

def pickleDecodeJson : String :=
  arr (pickleDecode.map fun (name, bytes) =>
    let result : Option (List (String × String)) :=
      match Pickle.decode bytes with
      | none => none
      | some es => es.mapM fun (m, v) => do
          let m ← Text.fromUtf16 m
          let v ← Text.fromUtf16 v
          pure (m, v)
    obj [("name", str name),
         ("hex", hex bytes),
         ("entries", opt (result.map fun es => arr (es.map fun (m, v) => arr [str m, str v])))])

/-! ## CF_HTML -/

def cfHtmlCases : List (String × String) :=
  [ ("empty", ""),
    ("ascii", "<b>hi</b>"),
    ("multibyte", "Café 🚀 <i>naïve</i>"),
    ("cjk", "<p>日本語</p>"),
    ("crlf", "a\r\nb") ]

def cfHtmlJson : String :=
  arr (cfHtmlCases.map fun (name, frag) =>
    let bytes := CfHtml.encodeString frag
    let o := CfHtml.offsets (Text.utf8 frag).length
    let payload := String.fromUTF8? (ByteArray.mk (bytes.map UInt8.ofNat).toArray)
    obj [("name", str name),
         ("fragment", str frag),
         ("payload", opt (payload.map str)),
         ("payload_hex", hex bytes),
         ("start_html", nat o.startHtml),
         ("end_html", nat o.endHtml),
         ("start_fragment", nat o.startFragment),
         ("end_fragment", nat o.endFragment)])

/-! ## RTF -/

def rtfEscapeCases : List (String × String) :=
  [ ("cafe", "Café"),
    ("rocket", "🚀"),
    ("cafe-rocket", "Café 🚀"),
    ("specials", "{a\\b}"),
    ("whitespace", "tab\tnew\nline"),
    ("carriage-return", "a\rb"),
    ("delete", "\x7f"),
    ("latin1", "ÿ"),
    ("bmp-max-positive", "翿"),
    ("bmp-min-negative", "耀"),
    ("bmp-max", "￿"),
    ("cjk", "日本") ]

def rtfEscapeJson : String :=
  arr (rtfEscapeCases.map fun (name, input) =>
    let units := Text.utf16 input
    let escaped := Rtf.escape units
    obj [("name", str name),
         ("input", str input),
         ("utf16", arr (units.map nat)),
         ("escaped", str (Text.ofCodes escaped)),
         ("ascii", bool (escaped.all (· < 128))),
         ("lean_roundtrip", bool (Rtf.unescape escaped == some units))])

/-- Unescaper inputs. `output` is `null` when Rust must reject. That covers bad escapes and
unpaired surrogates. -/
def rtfUnescapeCases : List (String × String) :=
  [ ("u233", "Caf\\u233?"),
    ("leading-zero", "\\u0065?"),
    ("surrogate-pair", "\\u-10179?\\u-8576?"),
    ("literal-unicode", "plain é literal"),
    ("escaped-specials", "\\\\\\{\\}"),
    ("min-negative", "\\u-32768?"),
    ("max-positive", "\\u32767?"),
    ("unescaped-open-brace", "{"),
    ("unescaped-close-brace", "a}"),
    ("unknown-control", "\\x"),
    ("missing-digits", "\\u?"),
    ("negative-zero", "\\u-0?"),
    ("positive-overflow", "\\u32768?"),
    ("negative-overflow", "\\u-32769?"),
    ("unterminated", "\\u12"),
    ("dangling-backslash", "\\"),
    ("sign-without-digits", "\\u-?"),
    ("double-sign", "\\u--1?"),
    ("sign-after-digit", "\\u1-2?"),
    ("lone-high-surrogate", "\\u-10179?"),
    ("lone-low-surrogate", "\\u-8576?x"),
    ("huge-digits", "\\u99999999999999999999?") ]

def rtfUnescapeJson : String :=
  arr (rtfUnescapeCases.map fun (name, input) =>
    let out := (Rtf.unescape (Text.utf16 input)).bind Text.fromUtf16
    obj [("name", str name), ("input", str input), ("output", opt (out.map str))])

def styledCases : List (String × Rtf.Run) :=
  [ ("bold-word", .seq (.text (Text.utf16 "Hello ")) (.group [.bold] (.text (Text.utf16 "world")))),
    ("nested", .group [.italic] (.seq (.text (Text.utf16 "a{"))
        (.group [.bold, .strike] (.text (Text.utf16 "b}🚀"))))),
    ("bare-group", .group [] (.text (Text.utf16 "plain"))) ]

def styledJson : String :=
  arr (styledCases.map fun (name, run) =>
    obj [("name", str name),
         ("rtf", str (Rtf.emitString run)),
         ("balanced", bool (Rtf.braceScan 0 false (Rtf.emit run) == some 0))])

/-! ## Spoiler -/

open Spoiler in
def spoilerCases : List (String × List Tok) :=
  [ ("valid", [.text "Hi ", .openTag .span true, .text "secret", .closeTag .span,
               .text " there"]),
    ("nested-span", [.openTag .span true, .openTag .span false, .text "a", .closeTag .span,
                     .text "b", .closeTag .span, .text "c"]),
    ("emphasis-inside", [.openTag .span true, .openTag .strong false, .text "x",
                         .closeTag .strong, .closeTag .span, .text "y"]),
    ("crossing-malformed", [.openTag .span true, .openTag .strong false, .text "x",
                            .closeTag .span, .closeTag .strong, .text "y"]),
    ("unclosed-malformed", [.text "a", .openTag .span true, .text "b"]),
    ("no-spoiler", [.openTag .strong false, .text "bold", .closeTag .strong]),
    ("plain-span-outside", [.openTag .span false, .text "p", .closeTag .span,
                            .openTag .span true, .text "q", .closeTag .span]) ]

def tagName : Spoiler.Tag → String
  | .span => "span" | .strong => "strong" | .em => "em" | .s => "s"

def tokJson : Spoiler.Tok → String
  | .text s => obj [("text", str s)]
  | .openTag t sp => obj [("open", str (tagName t)), ("spoiler", bool sp)]
  | .closeTag t => obj [("close", str (tagName t))]

def spoilerJson : String :=
  arr (spoilerCases.map fun (name, toks) =>
    let r := Spoiler.scan toks
    let reps := Spoiler.representations toks
    obj [("name", str name),
         ("tokens", arr (toks.map tokJson)),
         ("plain", str (Spoiler.renderPlain r.plain)),
         ("malformed", bool r.malformed),
         ("spoiler_omitted", bool reps.spoilerOmitted),
         ("rich", bool reps.signal.isSome)])

/-! ## Visibility -/

structure VisCase where
  name : String
  overrides : List (String × Bool)
  prefs : List String
  pins : Visibility.Pins
  app : Option String

def allHidden : List (String × Bool) := Visibility.allTargets.map fun t => (t.id, false)

def visCases : List VisCase :=
  [ ⟨"defaults", [], [], [], none⟩,
    ⟨"all-hidden", allHidden, ["telegram"], [], none⟩,
    ⟨"slack-alias", [], ["slack"], [], none⟩,
    ⟨"html-alias", [], ["html"], [], none⟩,
    ⟨"skip-hidden-and-unknown", [], ["word", "bogus", "discord"], [], none⟩,
    ⟨"enable-hidden-default", [("teams", true), ("rich-text", false)], ["teams"], [], none⟩,
    ⟨"visible-pin", [], ["telegram"], [("com.tinyspeck.slackmacgap", "slack-paste")],
      some "com.tinyspeck.slackmacgap"⟩,
    ⟨"hidden-pin", [], ["telegram"], [("com.microsoft.teams2", "teams")],
      some "com.microsoft.teams2"⟩,
    ⟨"pin-other-app", [], ["sms"], [("com.tinyspeck.slackmacgap", "slack-paste")],
      some "com.apple.MobileSMS"⟩,
    ⟨"pin-alias-not-resolved", [], [], [("com.example", "html")], some "com.example"⟩ ]

def visJson : String :=
  arr (visCases.map fun c =>
    let vis := Visibility.visibilityOf c.overrides
    obj [("name", str c.name),
         ("overrides", arr (c.overrides.map fun (k, v) => arr [str k, bool v])),
         ("preferences", arr (c.prefs.map str)),
         ("pins", arr (c.pins.map fun (k, v) => arr [str k, str v])),
         ("app", opt (c.app.map str)),
         ("visible", arr ((Visibility.visibleFormats vis).map fun t => str t.id)),
         ("preferred", str (Visibility.preferredFormat vis c.prefs).id),
         ("resolved", str (Visibility.resolve vis c.pins c.app c.prefs).id)])

def main : IO Unit := do
  IO.println (obj [
    ("schemaVersion", nat schemaVersion),
    ("generator", str "formal/lean: lake exe vectors"),
    ("pickle", pickleValidJson),
    ("pickle_decode", pickleDecodeJson),
    ("cf_html", cfHtmlJson),
    ("rtf_escape", rtfEscapeJson),
    ("rtf_unescape", rtfUnescapeJson),
    ("rtf_styled", styledJson),
    ("spoiler", spoilerJson),
    ("visibility", visJson)])
