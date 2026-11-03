---
id: WPOKE
title: BF1 MOVE OUT and AT EASE reach a waiting ground pilot and shorten its pickup window without overriding live board claims
status: archived
issue: 7
---

Historical technical record. Recorded state before archival was `parked`. Current scope and ownership are on [issue #7](https://github.com/sprucey149/openbf/issues/7).

## Why
The player cannot tell a newly seated BF1 bot to move out: its pickup wait
ignores both release orders. This is the remaining order-interruption slice
of GUNWT, after WAIT4/HWAIT/CWAIT supplied the initial and extended waits.

## Evidence
Existing `analysis/re-driver-wait.md` sections 2 and 7 specify BF1 MOVE OUT /
AT EASE setting the active wait window to now minus two seconds; a live team
board claim still holds the wait. Existing `analysis/re-squad-orders.md`
section 1, step 6 specifies the mounted receiver's threefold 3-D radius:
45 m becomes 135 m for these orders. This card inherits those documented
rules; it makes no new primary-data or live-console claim. BF2's human wait
and the velocity-vector hypothesis are outside scope.

On main 4730708, inline skiff/AT-ST fixtures naturally seat their pilot at
tick 1. Real MOVE OUT / AT EASE button edges at tick 31 reach zero pilots;
none answers or departs through tick 61 at either 8 m or 90 m. Eight BF1
scenes fail; forty BF2/out-of-range/enemy/no-order/live-claim controls pass.
`squad::in_reach` excludes every mounted unit and `obey` only releases an
existing Follow/Hold command. Neither path modifies the active pickup wait.
BF1-rule AT-ST is a port-only control, not a fielded BF1-class measurement.

## Scope
From an on-foot player, deliver only MOVE OUT / AT EASE to friendly AI pilots
in an active, matching hover/walker pickup wait within the documented mounted
radius. Preserve existing on-foot delivery and ordinary Follow/Hold release.
Do not broadly admit mounted units into `in_reach`: that helper also serves
FOLLOW, HOLD and dev commands. Other mounted orders, issuer-seat UI/delivery,
flyers, turrets, passengers and gunners remain separate original work.

Consume the pending order at the receiver's scheduled think, once per input
edge. Shorten the initial wait; do not directly force permanent completion
past a fresh eligible claim. Retain the actual seating/start clocks and the
vehicle identity, BF1 full-seat precedence, CWAIT freshness/eligibility and
permanent tenure completion. Model expiry without unsigned underflow on
early ticks. Hash any new future-affecting state and clear it through actual
entry/exit/reboard/switch/handover/slot-reuse paths. No synthetic completion
flag or unconditional no-claim diagnostic is the proposed implementation.

Preserve BF2 timing/human precedence, low-health bailout, gunner aim, route
progress/stuck allowance, claim allocation/give-up/order clocks and call/
reissue/answer-to-board policy. Expected release-order receiver/acknowledgement
events may change; do not add new calls, random draws or unrelated event moves.
Current sim/trace pins remain exact. Update changed source module/CRATES rows
as usual. No gate/oracle/baseline/golden/threshold/approval edits.

## Acceptance
1. Actual Sim::step button-edge tests naturally seat hover/walker pilots and
   reproduce all eight parent BF1 failures: both orders, 8 m and 90 m. Before
   ordering, assert zero locomotion and active wait. After the next scheduled
   think (at most THINK_PERIOD plus two observable ticks), require exactly
   one pilot acknowledgement, completed shortened no-claim wait and nonzero
   actual driver locomotion; log receiver/departure ticks and positions.
   Retain the forty passing controls with nonzero executed tests. A setup or
   compilation failure is not the parent failure.
2. Independently discriminate 135 m inclusive/exclusive and 3-D versus XZ;
   other team, dead/non-AI, stale player/vehicle generations, on-foot normal
   order recipients, both orders, Hold/Follow, no edge/held/repeated edges,
   delayed receiver thinks, completed wait, wrong vehicle, passenger/gunner,
   turret, human pilot and mounted issuer controls. Only the intended active
   BF1 ground-pilot wait gains the shortened window. Cover the early tick
   below two seconds and natural successful boarding, not just record edits.
3. A naturally approaching maintained claimant keeps the poked pilot stopped
   until actual boarding/full completion. A separate pre-four-second scene
   loses claim eligibility or suppresses refresh past the existing 3 s expiry:
   depart within one applicable update, rather than waiting the old deadline.
   Distinguish original since from maintenance; retain give-up/block policy.
   Repeated orders cannot extend the window or restart a completed tenure.
   BF2 initial/15 s human/full-seat rules retain their current behaviour.
4. Verify actual lifecycle/write-back, hash and reachable continuations for
   any persistent deadline/flag. Two distinct nonidle values must hash apart;
   remove each serialized field independently. Mutate mounted delivery,
   shortened-window use, each order arm, radius multiplier, 3-D distance,
   team/rule guards, scheduled-think handling, live-claim precedence, no-claim
   expiry, repeated-order completion and lifecycle resets separately: each
   must fail an assertion, with restored code passing. Keep route/stuck,
   bailout, gunner aim and immediate RNG/event controls discriminating.
5. Focused squad/ride/hover/walker tests, complete core/game libraries, fmt,
   workspace all-target Clippy and available map/protect checks must pass.
   Full CI and unchanged applicable sim/sweep/arena/leak/behavior/bake/GPU/
   fidelity/parity gates remain required. Verify real BF1 hover/walker pickup
   scenes with both commands, near/extended/out-of-range and maintained/
   expired claims; compare per-map boarding/stall/finish rows over 32 maps
   x 8 seeds, normal and Hard. Classify real refusals by the existing rules.
6. Implementation/fixtures do not complete this card's real-content bars or
   any original GUNWT/UTALK/WAIT4/HWAIT/CWAIT/GUNWH acceptance/dependency/hold.
   GUNWT still owes BF1 call timing/reissue, answer-to-board/allocation,
   original primary verification and UTALK TAT1/NAB1 rate census.
