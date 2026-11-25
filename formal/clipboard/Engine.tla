-------------------------------- MODULE Engine --------------------------------
EXTENDS Naturals

CONSTANT MaxOpId

VARIABLES session, shortcut

vars == <<session, shortcut>>
Modes == {"Picker", "CopyAndPick", "CopyOnly"}
Bindings == {"default", "alternate"}
Nothing == "none"
Busy == session.phase # "idle"

Init ==
    /\ session = [phase |-> "idle", mode |-> "Picker", generation |-> 0,
                   pickerId |-> 0, opens |-> 0, opensAtStart |-> 0]
    /\ shortcut = [phase |-> "ready", configured |-> "default", enabled |-> TRUE,
                    registered |-> {"default"}, paused |-> Nothing,
                    pending |-> Nothing, previous |-> Nothing]

StartCopy(mode) ==
    /\ ~Busy
    /\ session' = [session EXCEPT !.phase = "copying", !.mode = mode,
                                   !.opensAtStart = session.opens]
    /\ UNCHANGED shortcut

\* A busy hotkey press makes no changes, including to the generation.
DropBusyHotkey ==
    /\ Busy
    /\ UNCHANGED vars

FinishCopy(result) ==
    /\ session.phase = "copying"
    /\ LET open == session.mode # "CopyOnly" /\ result # "error"
                   /\ session.generation < MaxOpId
       IN session' =
            IF open THEN [session EXCEPT !.phase = "picker", !.generation = @ + 1,
                                         !.pickerId = session.generation + 1, !.opens = @ + 1]
                    ELSE [session EXCEPT !.phase = "idle"]
    /\ UNCHANGED shortcut

PickerReply(id) ==
    /\ session.phase = "picker"
    /\ id = session.pickerId
    /\ session' = [session EXCEPT !.phase = "idle", !.pickerId = 0]
    /\ UNCHANGED shortcut

IgnoreStaleReply(id) ==
    /\ session.phase # "picker" \/ id # session.pickerId
    /\ UNCHANGED vars

BeginRecord ==
    /\ shortcut.phase = "ready"
    /\ shortcut' = [shortcut EXCEPT !.phase = "recording", !.registered = {},
                    !.paused = IF shortcut.configured \in shortcut.registered
                               THEN shortcut.configured ELSE Nothing]
    /\ UNCHANGED session

\* RegisterEventHotKey happens before retiring a live binding. When
\* recording, the previous binding has already been paused.
BindNew(candidate) ==
    /\ shortcut.phase \in {"ready", "recording"}
    /\ shortcut.phase = "recording" \/ candidate = "default"
    /\ shortcut' = [shortcut EXCEPT !.phase = "registered",
                    !.registered = @ \cup {candidate}, !.pending = candidate,
                    !.previous = IF shortcut.configured \in shortcut.registered
                                 THEN shortcut.configured ELSE Nothing]
    /\ UNCHANGED session

UnbindOld ==
    /\ shortcut.phase = "registered"
    /\ shortcut' = [shortcut EXCEPT !.phase = "ready", !.configured = shortcut.pending,
                    !.enabled = TRUE, !.registered = {shortcut.pending},
                    !.paused = Nothing, !.pending = Nothing, !.previous = Nothing]
    /\ UNCHANGED session

BindFailed ==
    /\ shortcut.phase \in {"recording", "ready"}
    /\ shortcut' = [shortcut EXCEPT !.phase =
                      IF @ = "recording" THEN "restoring" ELSE "ready"]
    /\ UNCHANGED session

CancelRecord ==
    /\ shortcut.phase = "recording"
    /\ shortcut' = [shortcut EXCEPT !.phase = "restoring"]
    /\ UNCHANGED session

Restore(result) ==
    /\ shortcut.phase = "restoring"
    /\ shortcut' = [shortcut EXCEPT !.phase = "ready",
                    !.registered = IF result = "ok" /\ shortcut.paused # Nothing
                                   THEN {shortcut.paused} ELSE {},
                    !.paused = Nothing]
    /\ UNCHANGED session

SetEnabled(enabled) ==
    /\ shortcut.phase = "ready"
    /\ shortcut' = [shortcut EXCEPT !.enabled = enabled,
                     !.registered = IF enabled THEN {shortcut.configured} ELSE {}]
    /\ UNCHANGED session

CompleteStart == \E result \in {"ok", "warning", "error"}: FinishCopy(result)
CompletePicker == PickerReply(session.pickerId)

Next ==
    \/ \E mode \in Modes: StartCopy(mode)
    \/ DropBusyHotkey \/ CompleteStart \/ CompletePicker
    \/ \E id \in 0..MaxOpId: IgnoreStaleReply(id)
    \/ BeginRecord \/ (\E candidate \in Bindings: BindNew(candidate))
    \/ UnbindOld \/ BindFailed \/ CancelRecord
    \/ \E result \in {"ok", "failed"}: Restore(result)
    \/ \E enabled \in BOOLEAN: SetEnabled(enabled)

Spec ==
    /\ Init /\ [][Next]_vars
    /\ WF_vars(CompleteStart) /\ WF_vars(CompletePicker)

TypeOK ==
    /\ session \in [phase : {"idle", "copying", "picker"}, mode : Modes,
                    generation : 0..MaxOpId, pickerId : 0..MaxOpId,
                    opens : 0..MaxOpId, opensAtStart : 0..MaxOpId]
    /\ shortcut \in [phase : {"ready", "recording", "registered", "restoring"},
                     configured : Bindings, enabled : BOOLEAN,
                     registered : SUBSET Bindings,
                     paused : Bindings \cup {Nothing}, pending : Bindings \cup {Nothing},
                     previous : Bindings \cup {Nothing}]

SingleFlight ==
    /\ (session.phase = "picker") = (session.pickerId # 0)
    /\ session.pickerId # 0 => session.pickerId = session.generation

MonotoneOpId ==
    [][session.generation' >= session.generation]_vars

GenerationAccountsForOpens ==
    /\ session.generation = session.opens
    /\ CASE session.phase = "picker" -> session.opens = session.opensAtStart + 1
          [] session.phase = "copying" -> session.opens = session.opensAtStart
          [] OTHER -> session.opens \in {session.opensAtStart, session.opensAtStart + 1}

CopyOnlyZeroPicker ==
    session.mode = "CopyOnly" => session.opens = session.opensAtStart

RecordingPausesBinding ==
    shortcut.phase \in {"recording", "restoring"} =>
        /\ shortcut.registered = {}
        /\ shortcut.paused # Nothing => shortcut.enabled /\ shortcut.paused = shortcut.configured

RegisterFirst ==
    shortcut.phase = "registered" =>
        /\ shortcut.pending \in shortcut.registered
        /\ shortcut.previous # Nothing =>
            shortcut.previous = shortcut.configured /\ shortcut.previous \in shortcut.registered

StableBindingMatchesConfig ==
    shortcut.phase = "ready" =>
        shortcut.registered = {} \/
            (shortcut.enabled /\ shortcut.registered = {shortcut.configured})

\* A bind failure while not recording preserves the live binding. A failed
\* recording and a cancelled recording restore the paused binding when the
\* backend can reacquire it and keep the persisted configuration either way.
FailureOrCancelPreservesConfig ==
    [][(BindFailed \/ CancelRecord \/ Restore("ok") \/ Restore("failed")) =>
       /\ shortcut.configured' = shortcut.configured
       /\ shortcut.enabled' = shortcut.enabled
       /\ IF Restore("ok") /\ shortcut.paused # Nothing
          THEN shortcut.registered' = {shortcut.paused}
          ELSE UNCHANGED shortcut.registered]_vars

BusyEventuallyClears == []<>(~Busy)

=============================================================================
