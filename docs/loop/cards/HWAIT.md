---
id: HWAIT
title: A new BF2 AI ground driver holds its pickup wait for an eligible human teammate
milestone: M3
value: 4
phase: build
status: done
deps: UTALK
files: engine/crates/core/src/ai/ride.rs, engine/crates/core/src/ai/ride/tests/human_wait_tests.rs, engine/CRATES.md
declares: sim; sweep
needs:
effort:
branch:
resume: (private workflow note omitted)
created: 2026-10-25
alpha: yes
tries: 1
---

## Why
A BF2 bot drives away while the human is approaching its vehicle. WAIT4's
initial four-second window is implemented; the documented human extension
is independently implementable without GUNWT's unfinished claims, calls,
orders and boarding-response changes. GUNWH remains the end-to-end parent.

## Evidence
Existing `analysis/re-driver-wait.md` sections 5 and 7, inherited by GUNWH:
BF2 tests the human before full occupancy, holding until 15 seconds from
the wait's start while a living same-team human is on foot within 150m and
either within 12m or facing the vehicle within 30 degrees. BF1 has no such
extension. Reuse that documented rule; this is not new primary research.

On main86e55fa, natural boarding through Sim::step at tick1 produces eight
violations: hover/walker x empty/full passenger seat x human8m away/100m
away facing the vehicle. By tick181 every failing case has locomotion
stick1. Empty-seat drivers first drive at123 and move12.511396m (hover) /
14.824981m (walker); full-seat drivers first drive at2 and move74.998268m /
55.792282m. All36 BF1 and BF2 far-facing-away/enemy/dead/mounted controls
pass. These are inline skiff/AT-ST scenes, not real-class acceptance; the
BF1-rule AT-ST is a port-only control.

The traced cause is ride::pickup_wait: it completes at full occupancy or
the four-second boundary without the human test. A temporary diagnostic
in that actual caller, before completion, passes the same44 scenes. The
planner restores all source; a diagnostic is not a reviewed implementation.
Six longer actual-step scenes also distinguish the parent:10s near-human
hold then15s expiry with empty/full seats, or boarding at tick301, both
vehicle types. All six fail on parent and pass with the diagnostic; from
start1, diagnostic first drive452 at expiry or302 after boarding, with
zero held stick and under0.017m travel through tick301.

## Acceptance (run and paste)
1. Actual Sim::step with natural AI boarding, BF2 hover and walker: repeat
   all eight failing parent scenes above and the36 controls. A qualifying
   human yields zero locomotion sticks and under0.1m horizontal travel for
   six seconds, with the pilot still aboard; full occupancy cannot bypass
   it. Assert successful entry, player slot/identity, alive/team/on-foot,
   positions/facing, wait start, sticks and position. No injected AI output
   or fake claim/clock. Also test the near arm facing away, the facing arm
   outside12m, beyond150m, off-axis cone controls and both boundary sides.
2. Continue actual dispatch through15s from the first seated think: depart
   at that boundary within one think, empty or full, without restarting the
   deadline each tick. Before expiry, player entry into the last seat ends
   the hold on the next applicable update when nobody else qualifies.
   Losing eligibility before/after4s returns to the correct initial/full
   completion behavior. Test full occupancy before the first think, genuine
   leave/reboard, handover/seat switch, later vacancy, vehicle/slot generation
   reuse and a human approaching after permanent completion. Healthy waits
   neither dismount nor mark the vehicle stuck or spend route progress.
3. Separate BF1, enemy, dead, mounted, AI/idle-controller and no-human
   controls. Re-read respawned player slots and cover a qualifying second
   player with slot0 ineligible (port-only multiplayer control). Keep human
   pilot inputs, turret behavior, low-health bailout and existing gunner
   aiming/claim holds. Assert event/RNG controls. Existing default sim/trace
   pins stay exact; any changed pin is a blocker, not permission to edit it.
4. Independent omission, wrong game/team/liveness/on-foot/slot, near-arm,
   far-arm/cone/range, full-seat-precedence, timer-length/restart and
   completion/lifecycle mutations must compile and fail discriminating
   assertions, with restored source passing. Test both sides of the OR
   separately. A universal hold fails departure controls. If persistent
   state is added, hash every future-affecting field and prove reachable
   different-clock continuations; reuse of existing state needs no fake
   hash change. Do not move pins, goldens, thresholds or approvals.
5. Focused ai::ride/ai::walker_drive/ai::hover_drive, full core/game libs,
   fmt, workspace all-target Clippy and available map/protection checks.
   Restore actual BF2 hover/walker maps, classes, missions and reference
   inputs: original GUNWH8m/30m-away/100m-facing scenes and before/after BF2
   boarding/stall/finish rows, all32maps x8seeds on normal and Hard, with
   BF1 controls. Run current full CI and unchanged applicable sim/sweep/
   arena/leak/behavior/bake/GPU/parity gates; classify refusals normally.
   Fixture passes do not earn those missing checks or parent acceptance.
