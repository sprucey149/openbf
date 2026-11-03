---
id: WGORD
title: BF1 gunner-call recipients temporarily board while retaining their existing Hold or Follow order
status: archived
issue: 4
---

Historical technical record. Recorded state before archival was `parked`. Current scope and ownership are on [issue #4](https://github.com/sprucey149/openbf/issues/4).

## Why and evidence
A BF1 pilot's existing need-gunner call receives Yes from nearby Hold/Follow
bots, but those bots receive no vehicle goal and never board. Existing
analysis/re-driver-wait.md sections3/4/7 gives every eligible friendly on-foot
recipient a board goal; analysis/re-squad-orders.md sections3/4/4.4 describes
that goal retaining the prior Hold/Follow goal behind it. This card inherits
that evidence, with no new primary console claim or cadence/velocity guess.

Current need_gunner_call excludes commanded recipients from pending_board;
ride::deliver_call rejects them again. squad::steer clears target_cp, and
both delivery and board_goal additionally require a route to that post.
Merely deleting the two command gates cannot implement this path. The prior
order must survive a temporary explicit boarding goal, without giving an
ordered bot an autonomous post or autonomous vehicle search.

## Scope
Implement only existing BF1 ground hover/walker need-gunner recipients who
already hold a player Hold/Follow order at issue time. Preserve the old goal
behind boarding, including its issuer, issued tick, Hold point/Follow handle
and meaningful scratch/release progress. Do not erase the order to masquerade
as a commander-pool bot, reset its release counter or fabricate a target post.
An explicit called vehicle is the board goal; retain current seat admission,
claim arbitration, maintenance/give-up and physical entry policy. Separate
explicit call validation from autonomous trip-to-post admission as necessary.
Keep current wait-start/first-departure sources, independent salted rolls,
issue-time3-D Yes membership and scheduled next-think target delivery.

When the temporary board goal ends, resume the still-valid retained command
under existing order semantics. A new player order, explicit release, issuer
or receiver death, generation reuse or target loss must not resurrect a stale
order or board goal. Use existing applicable release rules; do not add a new
timer, guessed console condition or automatic dismount. Keep autonomous
command-free boarding and all BF2/no-call streams unchanged. Preserve current
one-claim arbitration/capacity, human priority and all earlier pickup work.
Revise old ordered-recipient rejection assertions explicitly to this supported
policy; retain their separate new-order cancellation and admission controls.
Later driver reissues, expanded allocation, other mounted commands, flyers/
turrets, velocity hypothesis and full UTALK rates remain GUNWT.

## Acceptance
1. Permanent actual Sim::step matrix, hover/walker x BF1/BF2 x pool/Hold/Follow:
   reproduce current4faults/8controls then pass12/12. Natural pilot entry9,
   wait-start Call/Yes10, receiver scheduled think19; supported ordered
   recipient gets pending at10, no pre-think claim, claims at19 and physically
   enters the called gun seat. Public issue_command sets the order before
   simulation; no injected wait/claim/events. Pool BF1 enters83 on parent.
   Record actual parent/candidate handles/ticks; BF1-rule AT-ST is dispatch
   coverage, not a fielded BF1 observation. Also exercise button-edge orders,
   both unit-index orders and delayed recipients on first-departure calls.
2. Retained Hold/Follow survives success and each reachable failure: full or
   unarmed seat, competing older claim, no route to vehicle, stuck/give-up,
   target destruction/reuse, boarding then ordinary dismount. Check same Hold
   point, Follow leader generation, issuer/issued_tick and non-reset release
   progress; actual walking resumes toward the retained goal after failure/
   dismount when valid. No fictitious post or unavailable-seat entry. New
   Hold/Follow/release and issuer death before issue, pending delivery, during
   approach and after boarding win under existing rules; no stale restoration.
3. Multiple recipients, repeated/late calls, changing membership and expired
   claims retain current arbitration, independent original claim clocks and
   maintenance. Preserve BF2 silence, player/dead/enemy/mounted/out-of-range
   exclusions,3-D boundary, failed rolls and wait/departure timing. New
   order installation still cancels pending/active explicit boarding. No
   command-free autonomous behavior change, extra holds, shared RNG draw or
   exact default sim/trace-pin update. Hash all newly persistent future state;
   demonstrate field sensitivity, identical repeats and lifecycle continuation.
4. Independent compiled command-gate, target-post-dependence, discarded-order,
   stale restoration/new-order priority, premature consumption and un-hashed-
   state mutants fail assertions. Restore/rebuild. Under shared CI lock run
   focused ride/squad/command/hover/walker/seats plus full core/game libraries,
   fmt, workspace all-target Clippy, map/protect and all available checks.
   Keep CRATES/header ownership true and every existing approval/gate intact.
5. Actual BF1 TAT1/NAB1 paired parent/candidate with identical build/input SHAs:
   record original/new orders, issuer/receiver/vehicle generations, issue/Yes/
   think/claim/entry/termination/resumption ticks and seat/capacity/outcomes.
   Zero missing supported targets, lost valid prior commands, stale revivals,
   duplicate/wrong claims or forbidden entries. Run32maps x8seeds Normal/Hard,
   every round finishes; sort boarding/stall changes normally. Full unchanged
   CI/map/protect/sim/sweep/arena/leak/bake/GPU/golden/parity/behavior gates
   and all GUNWT/GBOARD/WGCAL/WGOUT/WGSEAT/UTALK original bars remain required.
