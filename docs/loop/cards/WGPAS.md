---
id: WGPAS
title: BF1 gunner-call answerers can board an available unarmed secondary seat without changing autonomous boarding
status: archived
issue: 5
---

Historical technical record. Recorded state before archival was `parked`. Current scope and ownership are on [issue #5](https://github.com/sprucey149/openbf/issues/5).

## Why and evidence
An existing BF1 ground need-gunner call reaches an on-foot AI and receives
Yes, but the answerer cannot board a free unarmed secondary seat. The same
failure occurs when CanEnter's first free seat is unarmed and a later seat
has a gun. The call is already independent of the vehicle's armament; its
explicit board goal still falls through autonomous gun-seat admission.

Inherited evidence only: analysis/re-driver-wait.md sections3/4/7 describes
the answerers' goal5 for the called vehicle and its boardable seats;
analysis/re-squad-orders.md sections3/4/4.4 describes vehicle-goal stacking.
No new primary console claim, seat-count formula, velocity interpretation or
call cadence. At parent818ecf5, ride::claim_seat accepts CanEnter's first seat only if pilot or
armed; delivery, upkeep, driver hold and physical ai_use_pass all use it.
Changing delivery alone therefore cannot carry an unarmed claim to entry.

## Scope
Only existing BF1 ground hover/walker need-gunner recipients may use ordinary
CanEnter's available unarmed secondary seat. Include uncommanded recipients
and WGORD's retained Hold/Follow recipients. Carry explicit call provenance
through scheduled delivery, claim maintenance, pickup hold and physical
entry, clearing it at real termination/lifecycle writers. Hash any persistent
future state and preserve absent/idle streams. Keep one live claim per vehicle
under existing arbitration, all eligibility/team/boardable/capacity checks,
human priority, search/route rules, claim freshness/give-up and original clocks.
Do not treat every BF1 claim or every commanded bot as a call recipient.

Autonomous boarding, player/follower entry and all BF2/no-call behavior retain
their existing rules. Keep WGORD authoritative orders/release progress and
new-order precedence. Retain issue-time Yes, next-think delivery, source
timing/chance, both independent attempts and failed-roll consumption. Explicitly
replace old unarmed-call rejection assertions with correct supported-policy
checks; keep their independent no-call/full/nonboardable/new-order controls.
No blanket armament-gate deletion. Multiple simultaneous claims, expanded
allocation, later driver reissues, flyers/turrets, mounted player commands and
the velocity hypothesis remain GUNWT. Preserve all original parent acceptance.

## Acceptance
1. Permanent actual Sim::step matrix: hover/walker x BF1/BF2 x pool/Hold/Follow
   x armed gun/unarmed gun/passenger/unarmed-first-plus-armed-second. Reproduce
   current18missing-claim/entry failures and30controls, then pass48/48. All
   seat rings must satisfy is_ring. Use public pre-simulation orders, natural
   pilot entry9, BF1 Call/Yes/pending10 and receiver next_think19. Supported
   recipients claim at19 and physically enter the first boardable secondary
   seat; no injected claims/events/wait clocks. Retain original command each
   tick. BF1-rule AT-ST is port dispatch coverage, not a fielded observation.
2. Distinguish explicit calls from autonomous/no-call cases, including failed
   rolls, BF2, player/enemy/dead/mounted/out-of-range units, unavailable/full/
   nonboardable seats, unsupported kinds/classes, both unit-index orders,
   competing older claims and repeated calls. Keep one winner and no forbidden
   or duplicate entry; a later armed seat must not bypass CanEnter seat order.
   Retain human-seat precedence and actual full-seat departure. Include both
   wait-start and first-routed-departure sources with late recipients.
3. Keep explicit admission through real walking/maintenance and ai_use_pass;
   demonstrate held pickup past the initial window while a valid unarmed-seat
   boarder approaches. Repeated calls cannot reset since/maintenance/tenure or
   bypass a give-up/cooldown. New order/release, give-up, lost eligibility,
   target/receiver destruction or generation reuse, entry and ordinary
   exit clear only appropriate transient state; no later autonomous claim
   inherits it. Preserve existing issuer-loss policy. Retained valid orders
   resume, with release progress preserved.
   Hash every new persistent field/component, show identical continuation and
   preserve unaffected exact sim/trace/RNG pins without updating their values.
4. Independent compiled delivery-armament, upkeep-armament, pickup-hold,
   physical-entry-armament, global-BF1-admission, provenance-reset/new-order,
   arbitration/capacity and omitted-hash mutants must fail assertions. Restore
   and rebuild. Under shared CI lock run focused ride/squad/command/hover/
   walker/seats, full core/game libs, fmt, workspace all-target Clippy/build,
   map/protect and all available checks. Keep headers/CRATES ownership true.
5. Paired actual BF1 TAT1/NAB1 class/map inputs, exact parent/candidate builds:
   census fielded unarmed secondary seats first; record generation handles,
   seat kind/boardability/armament/occupancy, call/Yes/think/claim/maintenance/
   entry/departure/termination and retained-order rows. Zero supported missing
   goals/entries, stale goals, lost valid orders, duplicate claims or forbidden
   seats. Do not label fixture-only seat layouts as fielded. Run32maps x8seeds
   Normal/Hard, all finish; classify boarding/stall changes normally. All full
   unchanged CI/map/protect/sim/sweep/arena/leak/bake/GPU/golden/parity/behavior
   gates and original GUNWT/UTALK rates/restoration holds remain required.
