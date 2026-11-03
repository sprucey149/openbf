---
id: WGNXT
title: BF1 command-walker pilots call for riders again when setting out after a captured-post stop
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why and inherited evidence
A command walker's pilot stays seated at its post. When a teammate captures
that post, the pilot sets out toward the next one but never attempts another
need-gunner call. The existing first-departure bit remains spent for the
entire seating tenure, even after this real stop and new post departure.

analysis/re-driver-wait.md sections4/7 describes the seated trooper's FOLLOW
to type4 conversion before the existing0.3 roll, including a new trooper agent
after a post. Sections0/4/8 and analysis/re-squad-orders.md sections1/4.4/5
identify the source, issuer vehicle and recipients. Command-walker retention
is already implemented in ride::command and walker_drive::drive, documented
by analysis/re-walker-ai.md section13 and card WAI2. This is inherited research,
not a new primary-console or fielded-class claim.

## Scope
Only a BF1 command-walker AI pilot's subsequent set-out from an actually
reached post toward an accepted next post goal, within the same seating tenure.
Observe the real driver/goal transition; neither post ownership alone, target
changes during ongoing travel, node advance, turn rate nor velocity is the
source. Wait-start and first routed departure keep their independent existing
attempts. Every later witnessed post departure consumes one attempt even when
its roll fails; continuous driving/turning/stopping cannot retry it.

Publish through the existing deterministic after-all-write-backs path, using
the current salted0.3 call and current 3-D issue-time recipients/Yes/deferred
board delivery, retained commands, unarmed admission, source claim budget and
physical CanEnter. Full/unarmed/single seats do not gate the call. No second
pickup wait, restarted seating/claim clocks, extra RNG draw, altered steering,
new chance/cadence/range, seat allocation or voice-property change. Preserve
BF2, autonomous boarding, ordinary foot FOLLOW/MOVE OUT, hover/flyer/turret,
nonpilot/human and all existing bailout/command/claim precedence.

Keep first-departure history distinct from any subsequent-post state. A
planner diagnostic that clears PickupWait.departed while at the post only
proves the missing transition; do not ship it as the production design.
Hash every added persistent future-affecting field conditionally, including
two distinct non-idle values, with unchanged unaffected exact pins/RNG.
Later combat-agent/other set-outs, hover reissues, velocity interpretation and
original GUNWT/UTALK rate acceptance remain GUNWT. Do not generalize to unproved
transitions or claim complete reissue parity from this slice.

## Acceptance
1. Permanent natural Sim::step48-scene matrix: BF1/BF2 x capture/no-capture x
   pilot index0/1 x capture_time12/16/20/24/28/32s. Use the existing AT-ST dispatch
   fixture with command spawn points, two posts (12,0,-60)/(12,0,-300), and an
   actual on-foot player capturing the first. Natural pilot entry, actual
   first think, completed initial window, first route, sustained at-post stop,
   capture, retarget and accepted next driving goal; no injected claim, event,
   pending goal or pickup clock. Reproduce6parent missing-call assertions and
   42controls, then48/48. BF1 wait1/first121; successful later draws at p0t600,
   p1t609,p1t759,p0t870,p1t879,p0t990. Log real capture/restart and call rows.
   Count accepted route via complete pickup + driver route branch, excluding
   entry-tick Fwd and in-place post turning. Require the post was reached and
   stood at before the restart. All no-capture and BF2 controls stay silent
   after their existing source attempts. These classes/rings are synthetic.
2. Repeated real post/capture departures, successful and failed independent
   rolls; no repeated attempt on successive route ticks, delayed thinks, a
   changed target during continuous travel, route-node change, mere ownership
   change, unavailable route, post hold or turning without a new departure.
   Preserve first-departure history, seating/initial wait, held claim clocks,
   movement/events outside the added calls, shared RNG and default exact pins.
   Cover full/unarmed/single capacity, actual bailout, death/reentry/handover,
   unit/vehicle generation reuse, goal loss and changed authoritative orders.
3. Late pool/Hold/Follow recipients on both sides of the issuer index receive
   one Yes and an exact vehicle target after all writes, consume on their
   measured next_think and use existing budget/admission/physical entry rules.
   No lost retained command, forbidden/duplicate seat, stale target revival,
   duplicate goal, shared-clock reset or stranded completed pickup. Outside
   30m, vertical range, enemy/dead/mounted and failed-roll controls remain.
4. Independently compiled omission, every-tick reroll, ownership-time/early
   retarget, duplicate-publication, failed-roll retry, stale-tenure/generation,
   receiver-write-back and each new hash-field omission variant must fail
   named assertions; restore/rebuild the control. No compile-error credit.
   Focused ride/hover/walker/squad/seats, full core/game libraries, fmt,
   workspace all-target Clippy, map/protect and all unchanged applicable gates.
5. Actual BF1 fielded command-walker post-stop/next-post scenes, using verified
   class/seat/mission setup and separate exact parent/candidate binaries. At
   least8paired seeds per fielded scene; retain call/Yes/goal/claim/entry/order,
   stop/departure/tenure/generation rows and zero qualifying missing/duplicate
   attempts. Original GUNWT/UTALK TAT1/NAB1 rates and restoration acceptance,
   all32maps x8seeds Normal+Hard boarding/stall/finish comparisons and full
   CI/sim/sweep/arena/leak/bake/GPU/golden/parity/behavior gates remain required.
   Sort unrelated refusals with the unchanged NOISE/EXPOSED/CAUSED rules.
