---
id: WGSEAT
title: BF1 ground pilots attempt existing gunner calls regardless of vehicle gun-seat availability
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why and evidence
The existing BF1 ground pilot call is gated on a free armed gun seat, and
its departure source also requires an armed secondary class. This suppresses
calls and nearby acknowledgements from full and unarmed vehicles. Existing
analysis/re-driver-wait.md sections4/7 explicitly distinguish the seated
issuer's gate from the vehicle's seats, including a full one-seat vehicle.
Section4 documents independent wait-start and seated set-out sources, each
using the existing0.3 roll. This card inherits that research; it makes no new
primary console claim. WAIT4/CWAIT/GBOARD/WGCAL/WGOUT implementations supply
current source timing, pickup precedence and deferred target delivery.

## Scope
Remove vehicle capacity/secondary-armament admission only for BF1 hover and
walker AI pilots at the existing WGCAL wait-start and WGOUT first accepted
routed departure. Keep current source moments, independent attempts, seeded
salt/chance and failed-roll consumption. A full single-seat vehicle can
complete pickup and route away on the start tick: preserve the two distinct
sources when both actually occur. A completed wait with no actual start or
accepted route does not manufacture a missing source. Do not add a wait,
clock, reissue cadence, shared RNG draw, stick change or expanded allocation.

Keep issue-time3-D Yes membership, deferred GBOARD next-think delivery,
player-order precedence and existing board eligibility/arbitration. A call
from a full/unarmed vehicle is no authority to claim or enter an unavailable
seat. Preserve non-ground entry policy, BF2 silence, human/gunner exclusions,
live generation/seating-tenure checks, bailout and route-failure precedence.
Update earlier full/unarmed no-call assertions explicitly to the supported
policy while retaining their independent timing/delivery/lifecycle checks.
The unarmed BF1 call/state stream is now affected; compare unaffected foot,
BF2 and existing armed-free controls against parent. Hash every affected
future state; preserve exact existing default sim/trace pins and all gates.
Later combat/post reissues, ordered goal stacking, expanded allocation,
flyers/turrets, velocity interpretation and full UTALK rates stay GUNWT.

## Acceptance
1. Permanent actual Sim::step fixtures reproduce the planner's64scenes:
   hover/walker x BF1/BF2 x armed-empty/armed-full/unarmed-pair/unarmed-single
   x natural entry phases9/10 and public entry at10/11 with due first think.
   Parent12missing-call faults/52passing controls -> candidate64/64. Observe
   actual seat/start/route/NeedGunner/Yes ticks and handles, no fabricated
   wait state/claims/events. Natural phase9: full/single start+departure10
   have two successful attempts; unarmed pair starts10/departs130. Public
   entry10: pair starts10/departs131, only start roll succeeds. Phase10/11
   failed-roll controls must not retry. BF1-rule AT-ST is port dispatch
   coverage, never evidence of a fielded BF1 class.
2. Exercise current delivery on the new full/unarmed calls with receivers
   in either index order, same/delayed thinks, multiple/currently ordered
   recipients, issue-time3-D boundaries, team/dead/human/mounted exclusions.
   Yes on issue, no pre-think claim, no overwritten target; at next think
   unavailable seats fail current admission without displacing occupants,
   exceeding capacity or changing player commands. Preserve existing armed-
   free boarding and claim-maintenance success, including late recipients.
3. Preserve no-repeat per source, independent success/failure draws, route-
   failure/at-post/turn-first/WPOKE/claim-delayed cases and real lifecycle:
   exit/reentry, handover/death, unit/vehicle generation reuse. No stale or
   wrong issuer/vehicle calls. BF2, human/gunner and non-ground behavior
   retain parent bytes; full completion never adds a hold. Resolve affected
   old negative policy tests without dropping their separate referees.
4. Independent compiled omission mutants for publication gate and departure
   armament gate must fail distinct principals. Catch over-broad BF2/non-ground
   admission, retry/duplicate sources, immediate recipient consumption,
   capacity bypass and stale tenure. Restore/rebuild. Under shared CI lock
   run focused ride/squad/hover/walker/seat tests, complete core/game libraries,
   fmt, workspace all-target Clippy, map/protect and all available checks.
   Exact default pins and unaffected hash/RNG streams hold; no guard,
   approval, threshold, baseline or golden changes. Keep headers/CRATES true.
5. Paired actual BF1 TAT1/NAB1 parent/candidate rows with identical inputs,
   build/input SHAs, pilot/vehicle/receiver handles, seat/weapon/capacity,
   start/route/attempt/Call/Yes/claim/boarding/outcome. Require supported calls
   with zero stale/duplicate/wrong targets or forbidden entries. Census real
   fielded full/unarmed/single cases; distinguish fixture-only categories.
   Run32maps x8seeds Normal/Hard, all rounds finish, boarding/stall moves
   classified normally; unchanged CI/sim/sweep/arena/leak/bake/GPU/parity/
   behavior gates and full GUNWT/GBOARD/WGCAL/WGOUT/UTALK acceptance remain.
