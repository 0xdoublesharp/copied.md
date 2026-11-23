------------------------------ MODULE Clipboard ------------------------------
EXTENDS Naturals

CONSTANTS MaxExternalCopies, CheckAfterClear

VARIABLES phase, revision, externalCopies, captured, prepared,
          observed, content, outcome, staleAtWrite

vars == <<phase, revision, externalCopies, captured, prepared,
          observed, content, outcome, staleAtWrite>>

Init ==
    /\ phase = "idle"
    /\ revision = 0
    /\ externalCopies = 0
    /\ captured = 0
    /\ prepared = 0
    /\ observed = 0
    /\ content = "source"
    /\ outcome = "none"
    /\ staleAtWrite = FALSE

Capture ==
    /\ phase = "idle"
    /\ phase' = "picker"
    /\ captured' = revision
    /\ UNCHANGED <<revision, externalCopies, prepared, observed, content,
                   outcome, staleAtWrite>>

Cancel ==
    /\ phase = "picker"
    /\ phase' = "done"
    /\ outcome' = "cancelled"
    /\ UNCHANGED <<revision, externalCopies, captured, prepared, observed,
                   content, staleAtWrite>>

Confirm ==
    /\ phase = "picker"
    /\ phase' = "convert"
    /\ UNCHANGED <<revision, externalCopies, captured, prepared, observed,
                   content, outcome, staleAtWrite>>

ConversionFailed ==
    /\ phase = "convert"
    /\ phase' = "done"
    /\ outcome' = "conversion-failed"
    /\ UNCHANGED <<revision, externalCopies, captured, prepared, observed,
                   content, staleAtWrite>>

BeforeClear ==
    /\ phase = "convert"
    /\ phase' = IF captured = revision THEN "clear" ELSE "done"
    /\ outcome' = IF captured = revision THEN outcome ELSE "stale-before-clear"
    /\ UNCHANGED <<revision, externalCopies, captured, prepared, observed,
                   content, staleAtWrite>>

Clear ==
    /\ phase = "clear"
    /\ phase' = "check-prepared"
    /\ revision' = revision + 1
    /\ prepared' = revision'
    /\ content' = "empty"
    /\ staleAtWrite' = (revision # captured)
    /\ UNCHANGED <<externalCopies, captured, observed, outcome>>

CheckPrepared ==
    /\ phase = "check-prepared"
    /\ observed' = revision
    /\ IF CheckAfterClear /\ observed' # prepared
       THEN /\ phase' = "done"
            /\ outcome' = "stale-after-clear"
       ELSE /\ phase' = "write"
            /\ outcome' = outcome
    /\ UNCHANGED <<revision, externalCopies, captured, prepared, content, staleAtWrite>>

Write ==
    /\ phase = "write"
    /\ phase' = "done"
    /\ content' = "converted"
    /\ outcome' = "copied"
    /\ staleAtWrite' = (staleAtWrite \/ (revision # prepared))
    /\ UNCHANGED <<revision, externalCopies, captured, prepared, observed>>

WriteRejected ==
    /\ phase = "write"
    /\ phase' = "done"
    /\ outcome' = "write-rejected"
    /\ UNCHANGED <<revision, externalCopies, captured, prepared, observed,
                   content, staleAtWrite>>

ExternalCopy ==
    /\ externalCopies < MaxExternalCopies
    /\ externalCopies' = externalCopies + 1
    /\ revision' = revision + 1
    /\ content' = "external"
    /\ UNCHANGED <<phase, captured, prepared, observed, outcome, staleAtWrite>>

Next ==
    \/ Capture \/ Cancel \/ Confirm \/ ConversionFailed \/ BeforeClear
    \/ Clear \/ CheckPrepared \/ Write \/ WriteRejected \/ ExternalCopy
    \/ /\ phase = "done"
       /\ UNCHANGED vars

Spec == Init /\ [][Next]_vars

TypeOK ==
    /\ phase \in {"idle", "picker", "convert", "clear", "check-prepared", "write", "done"}
    /\ revision \in 0..(MaxExternalCopies + 1)
    /\ externalCopies \in 0..MaxExternalCopies
    /\ captured \in 0..MaxExternalCopies
    /\ prepared \in 0..(MaxExternalCopies + 1)
    /\ observed \in 0..(MaxExternalCopies + 1)
    /\ content \in {"source", "external", "empty", "converted"}
    /\ outcome \in {"none", "cancelled", "conversion-failed", "stale-before-clear",
                    "stale-after-clear", "copied", "write-rejected"}
    /\ staleAtWrite \in BOOLEAN

AbortedBeforeClearLeavesClipboardAlone ==
    outcome \in {"cancelled", "conversion-failed", "stale-before-clear"}
        => revision = externalCopies /\ content \in {"source", "external"}

NeverWriteAfterAnObservedChange ==
    phase = "write" \/ outcome = "copied" => observed = prepared

AbortAfterClearPreservesNewCopy ==
    outcome = "stale-after-clear" => content = "external"

\* Expected to fail even with both checks. Another process can copy
\* between the first check and clear or between the second check and write.
NoExternalRace == ~staleAtWrite

=============================================================================
