------------------------------- MODULE WinBoard -------------------------------
EXTENDS Naturals

CONSTANT MaxExternalCopies

VARIABLES phase, lock, revision, captured, content, externalCopies, staleAtWrite

vars == <<phase, lock, revision, captured, content, externalCopies, staleAtWrite>>

Init ==
    /\ phase = "idle"
    /\ lock = "none"
    /\ revision = 0
    /\ captured = 0
    /\ content = "source"
    /\ externalCopies = 0
    /\ staleAtWrite = FALSE

\* The snapshot and its GetClipboardSequenceNumber come from the same read.
Capture ==
    /\ phase = "idle"
    /\ lock = "none"
    /\ phase' = "ready"
    /\ captured' = revision
    /\ UNCHANGED <<lock, revision, content, externalCopies, staleAtWrite>>

OpenClipboard ==
    /\ phase = "ready"
    /\ lock = "none"
    /\ lock' = "engine"
    /\ phase' = "compare"
    /\ UNCHANGED <<revision, captured, content, externalCopies, staleAtWrite>>

Compare ==
    /\ phase = "compare"
    /\ lock = "engine"
    /\ phase' = IF revision = captured THEN "clear" ELSE "stale"
    /\ UNCHANGED <<lock, revision, captured, content, externalCopies, staleAtWrite>>

EmptyClipboard ==
    /\ phase = "clear"
    /\ lock = "engine"
    /\ phase' = "write"
    /\ revision' = revision + 1
    /\ content' = "empty"
    /\ staleAtWrite' = (revision # captured)
    /\ UNCHANGED <<lock, captured, externalCopies>>

SetClipboardData ==
    /\ phase = "write"
    /\ lock = "engine"
    /\ phase' = "committed"
    /\ revision' = revision + 1
    /\ content' = "converted"
    /\ staleAtWrite' = (staleAtWrite \/ revision # captured + 1)
    /\ UNCHANGED <<lock, captured, externalCopies>>

CloseClipboard ==
    /\ phase \in {"stale", "committed"}
    /\ lock = "engine"
    /\ phase' = "done"
    /\ lock' = "none"
    /\ UNCHANGED <<revision, captured, content, externalCopies, staleAtWrite>>

ExternalOpen ==
    /\ lock = "none"
    /\ externalCopies < MaxExternalCopies
    /\ lock' = "external"
    /\ UNCHANGED <<phase, revision, captured, content, externalCopies, staleAtWrite>>

ExternalCopyAndClose ==
    /\ lock = "external"
    /\ lock' = "none"
    /\ externalCopies' = externalCopies + 1
    /\ revision' = revision + 1
    /\ content' = "external"
    /\ UNCHANGED <<phase, captured, staleAtWrite>>

Next ==
    \/ Capture \/ OpenClipboard \/ Compare \/ EmptyClipboard \/ SetClipboardData
    \/ CloseClipboard \/ ExternalOpen \/ ExternalCopyAndClose
    \/ /\ phase = "done"
       /\ UNCHANGED vars

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ phase \in {"idle", "ready", "compare", "clear", "write", "stale", "committed", "done"}
    /\ lock \in {"none", "engine", "external"}
    /\ externalCopies \in 0..MaxExternalCopies
    /\ revision \in 0..(MaxExternalCopies + 2)
    /\ captured \in 0..MaxExternalCopies
    /\ content \in {"source", "external", "empty", "converted"}
    /\ staleAtWrite \in BOOLEAN

EngineOwnsLockUntilClose ==
    (lock = "engine") = (phase \in {"compare", "clear", "write", "stale", "committed"})

NoExternalRace == ~staleAtWrite

StalePreservesExternalCopy ==
    phase = "stale" => content = "external" /\ revision # captured

=============================================================================
