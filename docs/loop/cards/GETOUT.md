---
id: GETOUT
title: A ground pilot's Left command tells the AI seats of its vehicle EVERYONE OUT, with a 20-second autonomous-seat-search cooldown and driver feedback
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why and evidence
Before eb7a51e, SQVEH's implemented follower and pilot Right paths left pilot
Left falling through `squad_pass` without an order despite its EVERYONE OUT
label. The source checkpoint implements this requirement; acceptance stays pending.

- `analysis/re-squad-orders.md` sections 1, 2 and 4.4: seated type 5 addresses
  the issuer's own nonhuman seats; each consumes an order at its next think.
  SCDriverGetOutSound is `moveout`; the driver's Left label is EVERYONE OUT.
- SQVEH's retained PS2 log corrects section 4.4's older summary: six presses
  evicted the AI gunner in 0.07–0.50 s. The stored `now + 20 s` blocks only
  OWN seat search; a new PILE IN at 15 s seated it again. Do not implement a
  blanket no-boarding interval from the older prose. Original raw OP5/L15
  trials remain required for real acceptance; this planner makes no new RE claim.
- SQACK's merged-game policy supplies both ground-rule sets. BF2 fixtures here
  establish port policy, not a claim that the BF2 console has BF1's commands.
- On 7ddd8bd, 16 actual Left-edge cases (Bf1/Bf2 x hover/walker x armed/unarmed
  x pilot-before/after-receiver index) emit zero Command/OrderTaken and never
  evict in 16 ticks. All 48 idle, on-foot Left and pilot Right controls pass.
  Public spawn/enter_vehicle creates legal seats; the recipient's next_think
  is 9 and Left is held three ticks. The human pilot stays seated in every
  mounted scene. Durable test-only patch: reports/20261026_0939_getout_probe.patch.

## Exact scope
Human ground hover/walker pilot Left, distinct command identity 5 while existing
0..4 remain exact. Bind recipients at issue time to AI seats of the issued
vehicle, not nearby foot bots, other vehicles or humans. Consume once at each
recipient's scheduled think and leave through normal exit placement/seat cleanup.
Do not inject invalid mounts or force occupants through geometry in production.
Use generation-safe receiver/vehicle/issuer identity and appropriate tenure
validation so delayed orders cannot evict a replacement occupant or a new ride.

Set the own-search deadline to take time plus 20 seconds (600 ticks at 30 Hz).
Preserve it across AI write-back, exit and settle; do not globally change the
existing failure/bailout BOARD_BLOCK or block explicit PILE IN at 15 seconds.
Trace autonomous ground and turret seat searches before choosing storage;
`ai/man.rs` offers turrets separately and must honor the own-search deadline;
state affecting future behavior must be hashed. Leave ordinary exits, bailout,
AI calls and uncommanded replay hashes unchanged. Supersede the issuer's older
boarding/follow business using the existing new-order replacement semantics:
EVERYONE OUT must not be undone by stale pending calls, claims or an old Follow
shortcut on the same tick. Test retained orders explicitly; do not silently
alter unrelated players' command ownership or general follower behavior.

Use existing Command/OrderTaken feedback: driver cross, Left highlight, 2 s
fade, `moveout` property and receiver `yes`, including an empty vehicle. Keep
voice queue/locks, missing-bank fallback and field/Right feedback unchanged.
Gunner orders, flyers/turrets as issuers and absent class/hero/droideka modeling
stay SQVEH scope; no parent acceptance or old validation debt is removed.

## Acceptance
1. Reproduce the committed 16-failure/48-control witness on the exact parent;
   candidate passes all 64. Permanent tests use actual pad edges and public
   seats, with one Command at tick 0, answer and physical exit at scheduled
   tick 9, and no human eviction. Nearby fixtures finish within one think
   (<=0.5 s), with both rulesets, vehicle kinds, armament and unit-index orders.
2. Add 0/1/multiple AI seats, human secondary seats, other vehicles/foot bots,
   delayed unequal thinks, held/released/repressed input and other player-slot
   controls. Exact take-time +600 deadline blocks legally eligible autonomous
   search just before expiry and permits it at expiry; normal tick stepping
   preserves it. Public PILE IN at +450 ticks physically seats the evictee
   before expiry. Include prior Hold/Follow, pending AI/player claims and a
   newer order; no stale immediate reseating or persistent false claim.
3. Kill/reuse issuer, receiver and vehicle slots before delivery; switch/exit/
   reenter seats and replace occupants. No order redirects to a new vehicle,
   generation or tenure. Hash each new future-driving field; repeat scripts
   identically, preserving all existing no-command pins. Independently compile
   omission, early-delivery, all-seats/human-eviction, wrong-vehicle, cooldown-
   omission/shortening, blanket-explicit-block and stale-generation/hash mutants;
   require behavioral failure and restored control pass for each relevant rule.
4. Actual Command-to-feed/mapping tests require Left/driver labels/2 s fade,
   moveout and yes, with zero receivers and field/Right controls. Focused squad/
   ride/command/seats and event_feed/event_sounds/squad_orders_ps2, full core/
   game/UI, fmt, workspace build/all-target Clippy and ordinary CI/map/protect
   plus applicable sim/sweep/arena/leak/bake/GPU/golden/parity/behavior gates
   pass. No threshold, pin, golden, baseline or approval changes.
5. Restore SQVEH OP5/L15 and actual BF1 NAB1 CW AAT plus BF2 ground scenes;
   paired parent/candidate runs with identical content confirm every AI exit
   <=0.5 s, no human eviction, take-time +20 s own-search suppression, explicit
   PILE IN at +15 s reentry, actual sound names and visible Left cross/fade.
   Retain BUILD/input SHAs, raw seat/claim/clock/event rows and captures. All32
   maps x8seeds Normal/Hard finish; classify changed rows under existing rules.
   Full SQVEH and PILEIN acceptance remains independent and incomplete.
