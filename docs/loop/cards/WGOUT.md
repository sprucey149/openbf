---
id: WGOUT
title: BF1 ground pilots attempt a gunner call on their first routed departure after pickup
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why
A BF1 ground pilot's initial pickup wait can call for a gunner, but its
first set-out after that wait makes no second attempt. With an armed free
gun seat it leaves without asking nearby teammates to board. This is the
first post-wait set-out slice of GUNWT's documented second call source.

## Evidence
Existing analysis/re-driver-wait.md sections0,4,7,8 identifies two independent
sources: wait entry and the seated trooper setting out. Section4 and
analysis/re-squad-orders.md section5's correction identify the seated FOLLOW
to type4 conversion before its0.3 roll; sections1/4.4 identify the vehicle and
recipient goal. This card inherits that research, with no new primary PS2
claim. WGCAL8912bc9 supplies the initial wait-start source; it does not supply
this departure source. Ground drivers return before the foot trooper's
NavTick::SET_OUT production in ai/behave/brain.rs.

## Scope
For BF1 hover/walker AI pilots, make one existing need-gunner attempt when
they first set out along their driving route after the initial pickup wait.
It is independent of the wait-start attempt, even if that roll failed or no
teammate was in reach. Identify actual routed departure, rather than every
completed wait, every subsequent tick, a timer alone or a new chance cadence.
Retain current salted NEED_GUNNER_CHANCE/free armed-gun predicate and current
issue-time 3-D recipient/Yes/GBOARD admission and claim rules. Preserving the
free-gun predicate is a scope boundary, not a new console rule. Do not add a
second wait, restart its clocks, consume shared RNG or alter driving sticks.

Publish after all AI write-backs in deterministic order, retaining both unit
index orders and generation/tenure validation. Preserve the initial WGCAL
source, foot FOLLOW/MOVE OUT, BF2, full/unarmed/no-pilot/non-ground gates,
player orders, claim maintenance, human holds and bailout priority. Failed
set-out rolls are attempts too; never retry them on subsequent driving ticks.
Prefer existing transition state; hash any new future-affecting state while
preserving idle/default bytes and exact sim/trace pins.

This card covers the first routed departure from this pickup tenure. Later
reissues after combat/post/other agent transitions, original no-seat-test
console policy, ordered-goal stacking, expanded allocation, flyers/turrets,
the velocity hypothesis and full UTALK rate acceptance remain GUNWT scope.
Do not claim complete second-source parity from this bounded repair.

## Acceptance
1. Permanent actual Sim::step witnesses: natural ai_use_pass and public
   enter_vehicle paths, hover/walker, initial next_think0,1,2,9,10,11,20.
   Reproduce the planner's24missing-event failures and16passing scenes on
   parent. Candidate40/40pass. Observe actual entry, wait start, pickup
   completion, first routed departure, call/Yes and identities; no injected
   events/claims/wait state or clock assignment. Show distinct successful
   wait-start and departure attempts (natural phase9: ticks10and130), failed
   departure rolls (natural10/11), and zero repeated attempts while driving.
   BF1-rule AT-ST remains port dispatch coverage, not fielded BF1 evidence.
2. Add a late arriving eligible receiver: absent at wait-start, within30m
   in3-D at departure, actual Yes and next scheduled-think called-vehicle
   claim, no pre-think action or overwritten target. Include either index
   order, multiple recipients/current arbitration, changing membership,
   initial-success/failure, live-claim delayed departure and actual WPOKE
   release. Preserve GBOARD's command/generation/clock/lifecycle referees.
3. No set-out call on mere completion without routed departure, failed route,
   at-post exit, bailout, full/unarmed/BF2/non-ground paths, or for a gunner/
   human pilot. Include turn-before-translation, delayed first think, actual
   exit/reentry/handover/death/vehicle-slot reuse. Keep one attempt per valid
   departure tenure; do not use speed alone to classify an agent's set-out.
   Update old tests that assume only initial calls explicitly, retaining
   their independent WGCAL timing/eligibility and duplicate-call assertions.
4. Independent compiled mutants must assert-fail for omitted/early/duplicate
   departure calls, retry-every-tick/failed-roll retry, wrong issuer/vehicle,
   wait-only/no-route calls, early receiver publication/consumption and
   stale-tenure reuse. Rebuild restored sources; compare unaffected foot/BF2/
   unarmed hash/RNG traces and keep exact sim/trace pins. Under shared CI lock
   run focused ride/squad/hover/walker, full core/game libraries, fmt, workspace
   all-target Clippy, map/protect and all available normal checks. Keep CRATES
   and module headers accurate; no gate/approval/threshold/baseline/pin edits.
5. Paired actual BF1 TAT1/NAB1 parent/candidate rows: build/input SHAs, pilot/
   vehicle/receiver handles, entry/start/completion/departure/attempt/Call/Yes,
   issue distances, scheduled claim, boarding and subsequent driving. Require
   no stale/duplicate/wrong goals, no extra holds and correct supported calls.
   Run32maps x8seeds Normal/Hard, all rounds finish, boarding/fidelity moves
   classified normally; unchanged CI/sim/sweep/arena/leak/bake/GPU/parity/
   behavior gates. Original GUNWT/GBOARD/WGCAL/UTALK Acceptance remains owed.
