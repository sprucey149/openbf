---
id: PILEIN
title: A ground-vehicle pilot's Right command orders nearby teammates to PILE IN, with the driver cross and need-gunner line
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why and evidence
SQVEH's follower boarding is implemented, but `squad_pass` still discards every
seated player's edge. A human pilot pressing Right cannot ask nearby bots to
board. This is existing missing functionality, split from SQVEH, not a new PS2
claim or a replacement for that card's acceptance.

- `analysis/re-squad-orders.md` sections 1, 2, 3 and 4.4: seated type 4 sends
  the issuer's vehicle to every eligible friendly AI on foot within 30 m in
  3-D, consumed at its next think; the human's line has no 0.3 AI chance.
  Section 2 gives SCDriverGetInSound, `needgunner`, the driver labels and 2 s
  cross. SQVEH's retained O1/OP4 live log reports free/full-seat cases.
- SQACK's existing merged-game decision uses BF1 squad orders on both games.
  BF2 coverage here is that port policy, not a claim about BF2 console orders.
- On 78459a0, 2 ground-rule sets x hover/walker x armed/unarmed secondary seats
  give eight pilot-Right faults: zero Command, zero OrderTaken, nobody boards
  in 60 ticks. Twenty-four controls pass: on-foot Right follows, pilot idle
  holds, existing follower-to-mounted-leader boards (all eight at tick 0).
  Principal recipients start on Hold, 4 m beside the vehicle, next_think 9;
  public player spawn/enter_vehicle and actual Sim::step pads drive the scene.
  Right is held for three ticks to expose duplicate-edge delivery.

## Exact scope
Human PILOT, ground hover/walker, Right only: issue once on the rising edge,
retain the issued vehicle's generation, deliver at the receiver's think and
use the existing physical entry/claim machinery with explicit player-order
provenance. Both ground-rule sets must work. Admit ordinary boardable armed
or unarmed secondary seats. Keep earlier Hold/Follow installed behind the
explicit boarding goal as in WGORD; a later explicit order cancels it.
Preserve capacity, team, human-seat protection, generation and CanEnter rules;
no global armament exemption, autonomous claim change or AI-call change.

Emit a distinct command identity without renumbering field kinds 0..3, use
existing OrderTaken acknowledgement flow, map its sound to the driver's
needgunner line through the existing faction/bank routing, and show the driver
cross (empty up/down, PILE IN right, EVERYONE OUT left) with Right highlighted
and the existing fade/count policy. Show/ask the player's feedback even with
no receivers or a full vehicle. Preserve current missing-bank fallback and
voice queue/locks; this slice does not repair all class-specific sound routing.

EVERYONE OUT, gunner commands, flyers/turrets, the 20 s eviction timer and
unimplemented hero/droideka eligibility remain SQVEH scope. The left label is
part of the documented driver cross, not a claim that eviction now works.
Do not add unsupported class guesses or change already implemented followers.
GUNWT's remaining reissues/rates and all existing acceptance holds stay intact.

## Acceptance
1. Reproduce eight principal failures and 24 passing controls on the exact
   parent; candidate passes all 32. Keep permanent tests in the existing test
   layout, not the temporary planner module. Require exactly one Command at
   the press and one OrderTaken per receiver at its scheduled think, actual
   correct-vehicle boarding within 60 ticks in these nearby fixtures, no
   forced entry or directly injected pending-board state in principal tests.
2. Add actual-edge tests for n recipients/k free seats (0, 1, 2), both handle
   index orders and delayed thinks: min(n,k) seats occupied in nearby scenes;
   full/unboardable/other-team/human/dead/mounted/out-of-3-D-range recipients
   never steal seats or leave persistent claims. No cap/line-of-sight gate on
   delivery itself. Test zero receivers, held/released/repressed Right,
   player slots, newer orders, prior Hold/Follow restoration, issuer/receiver/
   vehicle death and slot-generation reuse, and issuer vehicle changes before
   delivery (never redirect to its new vehicle). State affecting future ticks
   is hashed; repeated input runs match; existing exact pins remain unchanged.
3. A real Command-to-EventFeed/voice-mapping fixture proves driver labels,
   Right arm, 2 s fade, needgunner property and next-think answers; on-foot
   labels/lines remain exact. Compile independent omission, wrong target,
   early delivery, repeated held edge, BF2 rejection, unarmed rejection,
   stale-generation and hash-state mutants; each must fail the relevant
   assertion, with restored control passing. Preserve WG* AI-call/claim tests.
4. Focused squad/ride/command/seats and game event_feed/event_sounds checks,
   full core/game/UI libraries, fmt, workspace build/all-target Clippy and
   ordinary CI/map/protect plus applicable sim/sweep/arena/leak/bake/GPU/
   golden/parity/behavior gates pass. No pin/golden/threshold/approval edit.
5. With actual BF1 NAB1 CW AAT and BF2 ground scenes, replay SQVEH O1/OP4
   near-ring empty/full controls and verify actual names/answers, vehicle
   generations and first boarding <=2 s; farther receivers need the original
   approach behavior, not a fictitious 2 s guarantee. View driver cross and
   field controls, listen to mapped takes, retain BUILD/input SHAs and paired
   raw rows. Run all32maps x8seeds Normal/Hard with unchanged gates; no-command
   controls must remain identical. Class/console/other-command coverage and
   full parent SQVEH acceptance remain required before that parent is done.
