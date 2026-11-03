---
id: STCHG
title: AI seat gunners charge and release their charge weapons
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
A bot at a charge seat gun never presses FIRE, so it never fires. The shared
seat-gunner predicate rejects MaxPressedTime > 0, and seated dispatch clears
the charge deadline on every tick. Repair these caller paths using the
existing on-foot charge policy and documented weapon-dispatch precedence.

## Evidence
Existing `analysis/re-turret-ai.md` section 7 (C24-C28) documents charge before
TriggerSingle, shared by seated and foot controllables. Section 8 names the
Hoth dish-turret path; `analysis/re-ai-difficulty.md` 4.4 and `skill.rs` specify
the existing U(CHARGE_MIN, skill.charge_hi) x MaxPressedTime release policy.
This card inherits that research; no new primary-evidence claim is made.

On main606c670, a temporary test reuses man::tests::seat_gun_scene, real
Sim::step, successful public entry, natural perception and weapon updates.
At 35 m over300ticks, both rule sets see the enemy291ticks and stay mounted:
BF1 charge1.5s:0held ticks/0shots; BF2 charge2.0s:0/0. With charge disabled,
the otherwise equal TriggerSingle controls fire36 and25shots respectively.
Both missing-shot assertions fail on the parent. These are inline synthetic
callers, not measurements of the real Hoth class or original60m scene.

A temporary seat-only copy of the foot charge branch still produces BF1 only
2shots/BF2zero: ride::drive resets charge_until every tick. Removing that reset
as a diagnostic yields15/9shots while the tap controls remain36/25. All863 old
core tests pass in the first diagnostic; in the second only the old man exit2
fixture fails, because it deliberately uses a charge gun as a gun that cannot
fire. The new four-case diagnostic passes. All source restored byte-for-byte.

## Scope
Only enable charge-seat fire. Charge dispatch precedes TriggerSingle, using
existing reaction/alignment/range permission at charge start and the existing
foot release rule. Share that policy if practical while preserving foot
behavior and every uncharged seat path. Retain a deadline throughout the
seated tick write-back. Reset stale charge state at actual weapon/seat tenure
boundaries; cover entry, exit, seat switch and the local ai_use_pass copy.
Use the existing hashed AiState field; preserve hash layout and current pins.
Keep human seat input, tap/burst cadence, class defaults and generic held-path
policy intact. The latter two still belong to VTAT. Full VTAT acceptance stays.

Maintain the no-shot turret exit referee using a reachable depleted-ammunition
scene: it must still prove the45s/no-shot20s exit and a firing gun's retention.
Do not delete that test or change its production exit rule. Update touched
module headers and CRATES rows. Avoid introducing new constants, protected
edits, thresholds, baselines, goldens or exemptions.

## Acceptance
1. Both-rule real Sim::step seat scenes, stable visible target35m away,
   charge1.5/2.0s and TriggerSingle true: each fires at least once in10s,
   from a fresh idle seat weapon. Log charge deadline, FIRE/fire_cmd and
   shot-owner/class/tick. Prove a nonzero hold followed by an actual release
   shot; initial carried foot FIRE cannot count. Add a TriggerSingle-false
   charge control to prove charge dispatch is independent of tap selection.
   Parent must fail both charge cases with the uncharged controls passing.
2. Prove deadline stays fixed during one uninterrupted charge, releases on
   its scheduled tick and can start another cycle, using the existing skill
   draw formula. Exercise a changed skill/class duration. Assert lifecycle
   reset/no inherited foot or previous-seat deadline through real entry,
   exit/reboard and seat switching. No-target behavior matches the existing
   foot policy; do not invent cancellation semantics for a release weapon.
3. Separate predicate-rejection, per-tick-deadline-reset, tap-before-charge
   and never-release mutations each fail a relevant assertion. A fixed or
   repeatedly redrawn deadline and missing lifecycle reset must also be
   discriminated by reachable controls. Restore each mutation. Include an
   actual mobile gunner/pilot dispatch control for the shared ride path.
4. Preserve existing uncharged tap, counted and heat bursts; compare their
   shot/heat/pause traces to the parent. Human seat trigger behavior and foot
   charge remain. Keep the man no-shot exit test meaningful via depletion,
   with its exit-rule omission failing. Default sim/trace pins stay exact.
5. Focused gunner/man/ride/seat tests, full core/game libraries, fmt,
   workspace all-target Clippy and available map/protection checks pass.
   Full CI and unchanged sim/sweep/arena/leak/behavior/bake/GPU/parity gates
   remain required with only declared movement; no fixture earns content
   acceptance. Restore real Hoth/AT-AT/laser class bakes and missions: run
   VTAT's three original10s scenes, preserving its tap and cannon bars;
   census burst/tap/charge shots and completion for32maps x8seeds,256/256
   rounds finish, classify other moves NOISE/EXPOSED/CAUSED as usual.
6. This source slice earns no VTAT whole-card/default/held-path acceptance.
   Keep both cards incomplete until every applicable original check passes.
