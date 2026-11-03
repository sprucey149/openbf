---
id: WGCAL
title: BF1 ground pilots make their initial gunner call when their pickup wait starts
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why
The initial BF1 need-gunner call currently runs in ai_use_pass when the pilot
enters. The pickup wait starts on a later seated think. The call therefore
uses the entry tick's roll and recipient positions. Public AI seating starts
the same pickup wait but never reaches that entry-pass call. This is the
initial call-moment slice of GUNWT; changing call frequency and seat admission
remains with the parent.

## Evidence
Existing analysis/re-driver-wait.md sections1,2,4,7,8 identifies the wait's
enter as the first call source and explicitly corrects seat-taken timing.
Sections4/7 and analysis/re-squad-orders.md4.4 specify the vehicle carried by
the call and recipient goals. This card inherits that research; it makes no
new primary-evidence claim. WAIT4's PickupWait.started records the existing
first seated think. GBOARD already delivers accepted goals at receiver thinks.

## Scope
For BF1 hover/walker pilots whose existing pickup wait actually starts, issue
the initial call exactly once at that transition. Remove their corresponding
entry-pass call, retaining the current chance/salt, free armed-gun gate,
issue-tick Yes list and GBOARD admission/arbitration. Use actual pilot/vehicle
identities; never infer a start from every later thinking tick. A failed roll
is still one attempt. Preserve the wait clocks and completion precedence.

Publish calls and pending targets in a fixed deterministic order after the AI
state write-backs that could otherwise overwrite them. Cover either unit-index
ordering of caller and receiver. Keep receiver goal consumption on its next
scheduled think, with no immediate claim or pre-think movement. Do not draw
from either shared RNG. Prefer the existing wait-start transition; hash any
additional future-affecting state, preserving idle/default bytes and exact pins.

The original no-seat-test console rule, full/one-seat and unarmed calls,
seated set-out/reissue, ordered-goal stacking, expanded seat allocation,
unverified velocity interpretation, flyers and turrets remain GUNWT scope.
Preserving today's free-gun gate here is an implementation boundary, not a
new claimed PS2 rule. Do not alter non-ground entry-call behavior, player
orders, bailout, claim maintenance, pickup tenure or driver route progress.
Keep every original GUNWT and GBOARD Acceptance bar and restoration hold.

## Acceptance
1. Permanent actual Sim::step scenes reproduce the planner's natural boarding
   and public enter_vehicle paths for hover/walker, with successful and failed
   existing salted rolls at distinct entry/start ticks. Assert actual calls
   and Yes events at wait start, no entry-tick duplicate, no later re-roll,
   correct issuer/vehicle/receiver identities and the unchanged wait times.
   Include initial next_think phases0,1,2,9,10,11,20; do not manufacture events,
   claims, wait transitions or advance the clock by assignment. BF1-rule AT-ST
   is only port dispatch coverage, not a fielded BF1 measurement.
2. Retain BF2, unarmed/free-seat gates and full-seat departure controls. Add
   changing 3-D recipient membership between entry and start, receivers both
   before and after the pilot in the unit list, failed-roll non-retry, delayed
   pilot think, duplicate thinks, actual exit/reentry/handover/slot reuse,
   command/bailout priority and non-ground unaffected controls. For a successful
   call, follow Yes through GBOARD's next-think claim, actual boarding and
   full-seat departure. Update old entry-moment test assumptions explicitly;
   retain all their eligibility, identity, arbitration and lifecycle coverage.
3. Compile and run independent mutants for old entry-only timing, omitted call,
   duplicate entry plus wait calls, re-rolling each think, wrong target/issuer,
   write-back overwrite and immediate receiver consumption. Each intended
   referee must assert-fail; rebuild and rerun exact restored sources. Compare
   unaffected foot/BF2/hash/RNG traces and preserve current sim pins.
4. Available checks under the shared CI lock: focused ride/squad/hover/walker,
   full core/game libraries, fmt, workspace all-target Clippy and map/protect.
   Keep source modules and CRATES accurate; no gate, approval, golden, baseline,
   threshold or pin edits.
5. On actual BF1 TAT1/NAB1, paired parent/candidate rows must show seat tick,
   first seated think/wait tick, attempt/result, issue-time recipient distance,
   call/Yes, receiver think/claim, boarding and departure, with correct handles
   and no duplicates or overwritten goals. Run32maps x8seeds Normal/Hard,
   require all rounds finish and classify boarding/fidelity moves normally.
   Run unchanged CI and applicable sim/sweep/arena/leak/bake/GPU/parity/behavior
   gates. The complete original GUNWT rates/reissue/console policy remain owed;
   fixture success alone does not earn acceptance.
