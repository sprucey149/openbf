---
id: WGMUL
title: BF1 gunner-call receivers can concurrently approach the called ground vehicle within its live claim budget
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why and evidence
A BF1 ground pilot calls and both nearby on-foot answerers say Yes, but
ride::board_goal's universal older-claim veto drops the second answerer even
when two secondary seats are free. One soldier boards and one seat stays empty.
Existing WGPAS provenance now distinguishes those explicit goals from search.

Inherited evidence: analysis/re-driver-wait.md sections3/4/7. Section3 specifies
live same-team kind-1 records, generation checks and three-second freshness;
its free-claim budget is seat count minus live team claims minus seats held by
a PlayerController, clamped at zero. AI occupants are not subtracted: a two-seat
vehicle with its AI pilot offers two claims. Section4 gives every eligible call
recipient goal5 and admits as many claims as that budget permits. This card
makes no new primary console claim. Physical CanEnter remains a separate test;
claim count and empty physical seats are different quantities.

## Scope
Only existing explicit BF1 hover/walker call goals use the documented concurrent
claim budget instead of the universal one-claim veto. Count valid existing claims
and player-held seats, including competing ordinary claims, once per owner/target
generation. Resolve contenders deterministically using the existing since/index
priority, real think scheduling and write-back order. Keep zero-budget/full/
nonboardable/invalid-class refusals and ordinary CanEnter/seat-order/human priority.
Do not replace the source budget with empty-seat count, subtract AI occupants,
or admit an unlimited number of called goals. Physical entry rechecks capacity;
extra approach claims end normally once no seat can be entered.

Preserve autonomous, BF2, turret, player and follower claim/entry policy. Keep
called_board provenance, WGORD authoritative Hold/Follow and new-order priority,
original since/maintenance/give-up/cooldown, target/receiver eligibility and
generation checks, and shared pickup holding while a valid boarder approaches.
Do not mint duplicate claims, reset another claimant's clock or revive stale
state to fill a vacancy. No new call source, cadence/chance, velocity rule,
flyer behavior or console evidence; later reissues and original rates stay GUNWT.

## Acceptance
1. Permanent actual Sim::step32scene matrix: hover/walker x BF1/BF2 x one/two
   secondary seats x one/two receivers x armed/unarmed valid seat rings. Reproduce
   four current faults and28endpoint controls, then32/32. Natural pilot entry9,
   BF1 Call/Yes10, actual receiver due19/18; no injected claims/events/clocks.
   With two spare seats both supported receivers claim on their due thinks,
   overlap in approach and physically enter distinct seats. Current parent only
   receiver index2 claims18/enters80, index1 never claims; diagnostic yields both.
   One-spare-seat controls keep one physical entrant; BF2 stays silent/claimless.
2. Separately referee the exact budget at the caller, beyond endpoint occupancy:
   AI pilot vs human-held seats, at least three receivers, two/three-seat rings,
   exact capacity and capacity+1, ordinary older claim, same-tick tie and both
   issuer/receiver index orders. AI occupants must not reduce source claim slots;
   live claims and player-held seats do. Expired/invalid generation/duplicate
   records cannot consume slots. Observe actual next_think per receiver and
   actual write-backs; zero over-budget surviving claims or forbidden entries.
   Revise old explicit one-claim assertions with named budget expectations;
   preserve independent autonomous/BF2/turret one-winner controls unchanged.
3. Both existing wait-start/first-departure sources; pool/Hold/Follow recipients;
   failed rolls/no-call/team/range/dead/mounted/unsupported/full/nonboardable
   controls; seat taken by human during approach, shrinking capacity, simultaneous
   arrival, moving/changed vehicle, claimant give-up/death/reuse and target reuse.
   Remaining valid claims retain original clocks/order/progress; no duplicate
   seat, stale revival or lost valid command. Driver holds through a real walk
   past initial wait, then departs on normal completion; surplus boarders release.
   Any new persistent state is conditionally hashed with distinct non-idle values,
   identical continuation and unchanged unaffected exact sim/trace/RNG pins.
4. Independent compiled one-claim, unlimited-budget, empty-seat-budget,
   subtract-AI-occupants, omit-player-seat, ignore-live-claim/freshness/generation,
   same-tick arbitration and physical-capacity mutants must fail assertions.
   Restore and rebuild. Under shared absolute CI lock run focused ride/squad/
   command/hover/walker/seats, full core/game libs, fmt, workspace all-target
   Clippy/build and available map/protect checks. Keep headers/CRATES accurate.
5. Census actual BF1 TAT1/NAB1 fielded multi-secondary-seat classes before real
   parent/candidate trials; fixture rings/AT-ST BF1 dispatch prove no fielded
   layout. Record exact build/input SHAs, handles/seat setup/occupancy, Call/Yes/
   due/claims/freshness/budget/entry/hold/departure/termination/order rows. Zero
   supported missing concurrent approaches/entries, over-budget claims, duplicate
   or forbidden entry, stale revival or lost valid order. Run32maps x8seeds on
   Normal and Hard for exact revisions, every round finishes; classify boarding/
   stall moves normally. All unchanged CI/map/protect/sim/sweep/arena/leak/bake/
   GPU/golden/parity/behavior and original GUNWT/UTALK rate bars remain required.
