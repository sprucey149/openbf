---
id: VCOL
title: No vertical collision response for soldiers (LEAK census classes ceiling + floor): push_out drops the vertical, so a jump puts the head through roofs, eaves and deck undersides (ceiling: 656-14,332 runs per map) and the axis crosses upward faces (floor: 429-9,283) — add a ceiling stop and find the floor crossings' mechanism (ground snap? a walkable slope the ground clamp does not lift onto?), fix, drive both counts down
status: archived
issue: 98
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #98](https://github.com/sprucey149/openbf/issues/98).

## Why
LEAK census classes: `ceiling` = the soldier's axis crossed a downward-facing SOLDIER face (n.y <= -0.5) between two ticks — a head through a roof, an eave, the underside of a deck or stair; `floor` = it crossed an upward-facing one (n.y >= 0.5). core/src/collision/walls.rs push_out's own doc: 'the vertical component dropped — floors are the ground query's job and ceilings are ignored' [V]. The maintainer's KAS2 hut (JWAL) shows ceiling runs round its roof as well.

## Evidence
- Counts: leak_census stdout. bf1_kas2: a walk under the ramp at (22.39,18.37,145.21) crosses a flatter face at tick 7 with push 0 (LEAK_TRACE=22.39,145.21,walk).
- Sites file: LEAK_SITES=<file>, classes ceiling/floor.

## Acceptance
- A ceiling response for a soldier (a rising soldier stops under a face; PROVISIONAL, the PS2's is untraced; RE only if the answer changes what the player sees).
- The floor class's mechanism named from traces, fixed or explained.
- Census ceiling + floor counts down to an explained residue; declare leak/sweep/arena/sim.

## Planner note (2026-09-30, M-ALPHA)
`alpha: yes` — no soldier out of bounds: heads through roofs. Value 4 (kept). Showcase maps: BF1 NAB1, BF1 TAT1 (the port's `tat1`), BF1 YAV2, BF2 UTA1, BF2 MYG1, BF2 HOT1 (PLAN §Milestones M-ALPHA).

## AUDIT note (2026-09-30, LEAK → LEAK3)
The ceiling/floor counts come from the census's centroid sampler.
- Samples at body height (the audit's band sampler, `lead_tmp/audit-leak/`) find 2,446 ceiling and 1,469 floor sites
  more, at walls the census never drives at.
- Worst: BF2 UTA1 box 565 at (−218, 202–214) has two ceiling sites with 154 and 144 runs.
- Measure the acceptance with LEAK3's sampler.

## Planner 2026-10-03 03:35 - the ceiling class after JARC
JARC (67c539b, the PS2's take-off: every jump 0.26 m higher) took the leak census's **ceiling class 37,858 -> 45,892**
(+21 %; TAT2 2,376 -> 4,547, TAN1 688 -> 1,550) and push 430 -> 605; floor fell 5,089 -> 4,914. The rise was declared
and accepted there as this card's class. The Acceptance's "before" is now `lead_tmp/jarc/leak/` (the landed column),
not the numbers above. The maintainer has not reported a head through a ceiling; value stays 4.

## AUDIT JARC note (2026-10-03) - a jump from under a deck ends ON the deck
The ceiling class is more than a head through a roof. A soldier standing under a thin deck (a 0.2 m slab) who jumps
straight up, no stick, ends standing ON the deck when its top is up to **2.60 m (BF2) / 2.70 m (BF1)** over his
floor; before JARC 2.35 / 2.40 m. Three things add up: no ceiling stop (this card), the PS2's 0.26 m taller jump, and
the falling soldier's ground query, which keeps the whole 0.6 m step in the air (d3ff7dc) and lifts him onto the
deck he has passed through. Awnings, low roofs, catwalks and stair undersides at that height are exits from the
level's intended floor.
- Check: `lead_tmp/audit-jarc/tools/deck_test.rs.txt` (a scratch test for `core::sim::step`'s tests; outputs
  `lead_tmp/audit-jarc/reach_audit-jarc-{pre,land}-scratch.txt`, lines `AUDIT deck ... stick 0`).
- Add to the Acceptance: that test, committed, reads "-" at every height from 1.90 m up in both games (a jump from
  under a deck never ends on it).
- Card JLEDG holds the wall side of the same code (the ledge a jump mounts; the PS2's rule is measured there).

## Build 2026-10-22 (lane 1, branch vcol b3da13b) - built, gated, parked for the GPU
- `Sim::ceiling_stop` (core/src/sim/step.rs), after the wall pass: a rising airborne soldier; two rays from 1 cm under the
  old head - along the head's move (ceiling-like faces only, solved against the face's plane at the new XZ), then straight
  up the new axis (any face); feet drop to 1 cm under, never under the tick's start; vel.y 0. PROVISIONAL (the PS2's is untraced).
- Test `a_jump_from_under_a_deck_never_ends_on_it`: box slab + single face both windings, tops 1.90-3.20 m, both games,
  stick 0 / full: ok; with the call removed it fails (`Bf2 stick 0 deck 0 top 1.90: ended at y 1.9`).
- Leak census, 32 maps (null build = main's baseline byte for byte): ceiling 45,309 -> 8,750; tunnel 1,177 -> 954; push
  545 -> 505; floor 5,321 -> 5,338; closed 28,735 -> 31,400 (2,326 of the first +2,606 at sites main already marks closed:
  the runs end where the walk runs end, LEAK2's parity); died 59,912 -> 60,881 (bf1_bes2: 602 of 606 new deaths were
  ceiling runs, now they stay under the deck and fall off the lower platform). Files lead_tmp/vcol/leak_{null,b3}.txt.
- Floor: named from 4 traces - a steep upward face (n.y 0.51-0.73) crossed on a tick the wall pass pushes 0.17-0.60 m;
  the ceiling residue is mostly GROUNDED (step-up / slope under a low face, a head walked into a sloped underside). Both -> VCOL2.
- Not measured with LEAK3's band sampler (LEAK3 is a parked branch, not on main).
