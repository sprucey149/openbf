---
id: CWAIT
title: A ground AI driver keeps its pickup wait while a teammate maintains a live boarding claim
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
A bot walks toward an empty gunner seat, repeatedly thinks and keeps its
boarding claim, but the driver leaves before it arrives. The existing claim
hold measures eight seconds from the original claim, rather than its latest
maintenance. Walker dispatch can leave even earlier. This isolates GUNWT's
maintained-claim wait from its call/answer, order and allocation work.

## Evidence
Existing `analysis/re-driver-wait.md` sections 2, 3, 5 and 7 specify a ground
driver's wait after the initial window while a same-team boarding claim is
live, completion on full occupancy (after BF2's human extension), and claim
maintenance/freshness. Section 3 records the three-second lifetime and matching
entity identities; section 7 describes restamping on each think. This card
uses that existing research, without claiming a fresh primary verification.
The interpretation of the board agent's velocity vector remains [H] in the
note; neither that threshold nor additional original-game rules belong here.

On main20dd5a4, temporary Sim::step fixtures naturally board the pilot at
tick1, then spawn an AI teammate70m away. Existing claim selection acquires
the armed gunner seat at tick2. Both source rules, hover and walker, fail:
the boarder loses the claim at242 without boarding by601. It has25 scheduled
thinks while holding that claim. Hover first drives242; walker first drives
122(BF1)/123(BF2). Four otherwise equal no-boarder controls correctly depart
122/123. No injected boarding claim, clock, AI pad or enter request.

A temporary diagnostic suppresses only the old age-based abandonment and
holds in the shared pickup path while an on-foot AI has the existing claim.
All eight scenes pass: each long-approach boarder enters250, driver leaves251,
zero commanded locomotion before boarding; no-boarder controls are unchanged.
This deliberately incomplete diagnostic has no freshness/identity coverage
and is not a production implementation or an acceptance receipt.
The inline skiff/AT-ST scenes are port fixtures; BF1-rule AT-ST is port-only.

## Acceptance (run and paste)
1. Reproduce the eight scenes above on current main: four intended failures,
   four departure controls. Candidate: natural claims persist across8s;
   both ground vehicle types/rules wait with zero locomotion and under0.1m
   horizontal travel until boarding; the boarder reaches the gunner seat and
   full occupancy releases the driver within one think. Record actual claim,
   think/stamp, entry, wait completion, departure and position observations.
   Repeat with different valid think phases and a longer approach so the
   fixed8s cutoff and an accidentally refreshed start clock are distinguishable.
2. Discriminate freshness from total claim age: recent maintenance keeps an
   old claim live, unmaintained claims expire at the documented3s boundary
   within one think, and the initial4s window still works without claims.
   Preserve the original give-up clock and ordering; a perpetually refreshed
   claim cannot evade BOARD_GIVEUP. Cover no claim, stale/wrong vehicle or unit
   generation, dead/mounted/enemy/non-AI, full seats, multiple claimants and
   claim release/invalidity. State clearly which cases are direct rule tests
   and which are reachable real-step continuations; do not fake caller output.
3. Preserve HWAIT's human/full-seat precedence, permanent completion, leave/
   reboard, takeover, seat switch, slot reuse, bailout priority, gunner aiming,
   no event/RNG draw, and route/follower/stuck clocks. Migrate the explicitly
   legacy `pickup_legacy_claim_hold_survives_initial_expiry` referee to the
   documented replacement; do not drop its reachable driver coverage.
   All other WAIT4/HWAIT controls stay. New hash fields need field-by-field
   omission mutations and reachable different-clock future continuations.
4. Independent omission, hover/walker bypass, old8s hold, old8s abandonment,
   missing refresh, refreshed-since, wrong3s lifetime, wrong-team/identity,
   full-seat precedence, universal hold and completion/restart mutations
   compile and fail distinct assertions; restored source passes. Run focused
   ride/hover/walker, full core/game libs, fmt, workspace all-target Clippy and
   available map/protection checks under the normal shared lock/environment.
   No pin, threshold, golden, baseline, exemption or approval changes.
5. Restore real maps/classes/missions, original reference/census inputs and
   graphics prerequisites. Run current full CI and unchanged applicable sim,
   sweep, arena, leak, behavior, bake, GPU/parity gates. Record32maps x8seeds
   normal/Hard boarding/stall/finish comparisons and original GUNWT pickup
   scenes; classify refusals normally. GUNWT's TAT1/NAB1 call-rate acceptance
   and order/answer/allocation scope remain owed separately. Inline fixtures
   do not earn these real-content checks or acceptance of the parent card.
