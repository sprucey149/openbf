---
id: GBOARD
title: BF1 idle teammates that answer a need-gunner call receive its vehicle boarding goal at their next think
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why
A BF1 ground driver asks for a gunner; a nearby idle teammate says yes and
continues to its post. Its post is close enough to prevent autonomous vehicle
search, so the existing pickup wait cannot get a boarder. This is the
uncommanded receiver-to-board slice of GUNWT, separate from call timing,
reissue, seat-count policy and the original real-map voice-rate census.

## Evidence
Existing `analysis/re-driver-wait.md` sections3/4/7 and
`analysis/re-squad-orders.md` sections1/4.4 specify a type4 recipient's board
goal for the order's vehicle, taken at its next think. GUNWT's Acceptance
explicitly retains the port's claim arbitration: one takes the gun seat,
the rest lose their claims. No new primary-console claim or measured rate.

Before source48b1066, `Sim::need_gunner_call` emitted Call::Yes but stored no
vehicle goal.
`ride::claim` requires a trip over BOARD_TRIP for ordinary non-command
vehicles;
there was no call-goal input. Source48b1066 supplies that input as recorded
below. `board_goal` already maintains accepted claims,
checks the vehicle/seat/route and arbitrates by original since/unit index.
Use those existing rules, with CWAIT freshness and WAIT4 pickup hold.

## Scope
For an existing successful BF1 need-gunner call, give its otherwise eligible,
uncommanded on-foot AI receivers a generation-checked vehicle goal. Consume
it at each receiver's scheduled think, using existing boarding, routing,
seat eligibility and claim arbitration; do not install an immediate claim
in the issue loop. Preserve the current issue-tick yes events and their
recipient list. Do not add random draws or repeated yes/call events.

This card handles bots in the normal commander's pool. A receiver under a
player Hold/Follow command keeps the current behavior; ordered-goal stacking
and original-console excluded class coverage remain GUNWT scope. Do not
turn that implementation boundary into a new claimed PS2 rule. An intervening
player order must keep its normal priority and cannot be erased by this goal.
Call moment/chance/free-gun gate, seated set-out calls, BF2, multi-seat
allocation changes, the velocity-vector hypothesis, human squad UI, flyers
and turrets remain outside this slice. GUNWT's full Acceptance is unchanged.

Store only state needed to deliver the exact call target once; hash any new
future-affecting state while keeping idle/default hashes and current pins
unchanged. Reject invalid/stale target or receiver identities; discard pending
goals on actual receiver lifecycle/entry paths. A second delivery must not
reset the original claim's since/give-up clock or another vehicle's live claim.
No post-distance/cooldown bypass may leak into ordinary autonomous search.
Update source module headers and CRATES rows; split tests into a child file
if needed to respect the source-size gate. No gates, goldens, approval,
baseline, threshold or pin edits.

## Acceptance
1. Permanent actual `Sim::step` tests reproduce all four parent assertion
   failures: BF1 hover/walker, receiver20/25m away and <BOARD_TRIP from its post.
   Use the existing legal skiff/AT-ST fixtures and natural pilot boarding.
   Assert matching NeedGunner/Yes first, zero independent receiver claim before
   delivery, then the call-target goal at the next scheduled think, actual
   gunner boarding and driver departure when full. Log issue/think/claim/entry
   ticks and vehicle identity. BF1-rule AT-ST is a port-only dispatch control.
2. Keep the ten planner controls: failed coin, BF2, no armed gun, receiver
   outside30m and ordinary far-post autonomous boarding, both vehicle kinds.
   Add nonzero controls for 3-D reach, team/alive/player/mounted admission,
   commanded recipients/intervening player orders, two competing receivers,
   a nearer decoy vehicle, blocked own-search cooldown, repeat delivery,
   stale/destroyed/full/ineligible target and receiver respawn/slot reuse.
   Preserve current claim winner/loser policy; do not claim PS2 allocation.
3. Prove delayed delivery on receiver thinks, one board goal per accepted
   call, no pre-think movement/use caused by it and no extra Call/RNG draws.
   Independently mutate delivery omission, immediate consumption, wrong
   target/handle, autonomous-only post filter, arbitration, repeated clock
   reset and hash omission. Each mutant must compile, execute the intended
   referee and fail an assertion; rerun exact restored sources afterwards.
4. Available checks: focused ride/squad/hover/walker, full core/game libraries,
   fmt, workspace all-target Clippy and normal map/protect checks under the
   shared CI lock. Preserve exact sim/trace pins and existing WAIT4/HWAIT/
   CWAIT/WPOKE lifecycle, timer, bailout, full-seat and human controls.
5. With real maps/missions/UTALK kit restored, replay BF1 TAT1/NAB1 actual
   calls into board goals, showing natural calling/answering/boarding and
   parent/candidate/control rows. Run32maps x8seeds Normal/Hard, require every
   round finishes and classify boarding/fidelity changes using normal rules.
   Run unchanged CI, sim/sweep/arena/leak/bake/parity/behavior gates as applicable.
   The original GUNWT TAT1/NAB1 voice-rate bars remain with the parent; this
   slice does not claim to meet them. Synthetic pass alone is not acceptance.
