----------------------------- MODULE CopyAndPick -----------------------------
EXTENDS Naturals, Sequences

CONSTANT MaxExternalCopies

VARIABLES board, flow

vars == <<board, flow>>
Targets == {"rich", "plain"}
Modes == {"CopyAndPick", "Picker", "CopyOnly"}
None == "none"

\* Use a uniform record for the atoms and conv(target, src) so TLC can
\* compare values of different clipboard kinds.
Atom(kind) == [kind |-> kind, target |-> None, src |-> None]
conv(target, src) == [kind |-> "converted", target |-> target, src |-> src]
Atoms == {"source", "external", "empty"}
Payloads == {Atom(kind) : kind \in Atoms}
            \cup {conv(target, src) : target \in Targets, src \in Atoms \cup {"converted"}}
Revisions == 0..(MaxExternalCopies + 2)
Attempts ==
    [kind : {"auto", "pick"}, target : Targets, input : Atoms \cup {"converted"},
     expected : Revisions, seen : Revisions, afterRevision : Revisions,
     result : {"copied", "stale"},
     before : Payloads, after : Payloads,
     lastBefore : Targets \cup {None}, lastAfter : Targets \cup {None}]

\* Capture is the starting boundary. "source" names the text of this session,
\* including cases where another app put that text there before capture.
Init ==
    \E mode \in Modes, target \in Targets, pinned \in BOOLEAN, last \in Targets \cup {None},
       source \in {"source", "empty"}:
        /\ board = [revision |-> 0, content |-> Atom(source), copies |-> 0]
        /\ flow = [mode |-> mode, target |-> target, pinned |-> pinned,
                    last |-> last, original |-> source, captured |-> 0,
                    phase |-> "captured", attempts |-> <<>>]

Begin ==
    /\ flow.phase = "captured"
    /\ flow' = [flow EXCEPT !.phase =
          CASE flow.original = "empty" -> "empty"
            [] flow.mode = "Picker" -> "picker"
            [] OTHER -> "auto"]
    /\ UNCHANGED board

Attempt(kind, target, expected, remember) ==
    LET success == (expected = board.revision)
        payload == IF success THEN conv(target, flow.original) ELSE board.content
        revision == IF success THEN board.revision + 1 ELSE board.revision
        last == IF success /\ remember THEN target ELSE flow.last
        entry == [kind |-> kind, target |-> target, input |-> flow.original,
                  expected |-> expected, seen |-> board.revision,
                  afterRevision |-> revision,
                  result |-> IF success THEN "copied" ELSE "stale",
                  before |-> board.content, after |-> payload,
                  lastBefore |-> flow.last, lastAfter |-> last]
    IN
    /\ board' = [board EXCEPT !.revision = revision, !.content = payload]
    /\ flow' = [flow EXCEPT !.last = last,
                            !.attempts = Append(@, entry),
                            !.phase = IF success /\ kind = "auto" /\ flow.mode = "CopyAndPick"
                                      THEN "picker" ELSE "done"]

AutoCopy ==
    /\ flow.phase = "auto"
    /\ Attempt("auto", flow.target, flow.captured, ~flow.pinned)

Pick(target) ==
    /\ flow.phase = "picker"
    /\ IF flow.mode = "CopyAndPick"
       THEN /\ target # flow.target
            /\ Attempt("pick", target, flow.attempts[1].afterRevision, TRUE)
       ELSE Attempt("pick", target, flow.captured, TRUE)

Cancel ==
    /\ flow.phase = "picker"
    /\ flow' = [flow EXCEPT !.phase = "cancelled"]
    /\ UNCHANGED board

ExternalCopy ==
    /\ board.copies < MaxExternalCopies
    /\ board' = [board EXCEPT !.revision = @ + 1, !.copies = @ + 1,
                               !.content = Atom("external")]
    /\ UNCHANGED flow

Next ==
    \/ Begin \/ AutoCopy \/ (\E target \in Targets: Pick(target)) \/ Cancel \/ ExternalCopy
    \/ /\ flow.phase \in {"empty", "done", "cancelled"}
       /\ UNCHANGED vars

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ board \in [revision : Revisions, content : Payloads, copies : 0..MaxExternalCopies]
    /\ [flow EXCEPT !.attempts = <<>>]
         \in [mode : Modes, target : Targets, pinned : BOOLEAN, last : Targets \cup {None},
                 original : Atoms, captured : Revisions,
                 phase : {"captured", "empty", "auto", "picker", "done", "cancelled"},
                 attempts : {<<>>}]
    /\ Len(flow.attempts) <= 2
    /\ \A i \in 1..Len(flow.attempts): flow.attempts[i] \in Attempts

NeverConvertAConversion ==
    \A i \in 1..Len(flow.attempts):
        LET write == flow.attempts[i] IN
        write.result = "copied" =>
            write.input = "source" /\ write.after = conv(write.target, "source")

SecondWriteChecksFirstRevision ==
    Len(flow.attempts) = 2 =>
        LET first == flow.attempts[1]
            second == flow.attempts[2]
        IN /\ first.kind = "auto" /\ first.result = "copied" /\ second.kind = "pick"
           /\ second.expected = first.afterRevision

ExternalCopyBetweenWritesAbortsCleanly ==
    Len(flow.attempts) = 2 =>
        LET first == flow.attempts[1]
            second == flow.attempts[2]
        IN second.seen > first.afterRevision =>
           /\ second.result = "stale"
           /\ second.before = Atom("external")
           /\ second.after = Atom("external")
           /\ board.content = Atom("external")
           /\ board.revision = board.copies + 1

CancelAfterAutoCopyPreservesAutoCopy ==
    flow.phase = "cancelled" /\ flow.mode = "CopyAndPick" =>
        LET first == flow.attempts[1] IN
        /\ Len(flow.attempts) = 1
        /\ first.result = "copied"
        /\ board.revision = board.copies + 1
        /\ board.content = IF board.copies = first.seen THEN first.after ELSE Atom("external")

PinnedAutoCopyNeverRewritesLastTarget ==
    flow.pinned =>
        \A i \in 1..Len(flow.attempts):
            LET write == flow.attempts[i] IN
            write.kind = "auto" => write.lastBefore = write.lastAfter

=============================================================================
