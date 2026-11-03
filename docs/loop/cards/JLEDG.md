---
id: JLEDG
title: The ledge a jump mounts: on the PS2 a BF2 jump pressed against a face gets on only with its apex 0.42 m over the top (1.44 m yes, 1.51 m no; 25 trials), a BF1 jump with it up to 0.15 m under (2.07 m yes, 2.17 m no; 16 trials). The port's BF1 agrees (2.16 m); its BF2 mounts 1.75 m through AIR_STEP's mantle (ours) - measure the running jump, then make BF2's rule the PS2's
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
JARC bounded the jump's mantle with `collision::AIR_STEP` (0.3 m, ours) from one PS2 observation (a BF1 jump 0.35 m
short of a 2.26 m wall) and wrote "the PS2 has no step at all". AUDIT JARC (2026-10-03) measured the rule itself:
- **PS2, BF2**: a jump pressed against a vertical face mounts the ledge only when the feet's apex is 0.42 m or more
  over the top (13 of 13) and never at 0.35 m or less (12 trials): a 1.861 m jump gets onto a 1.44 m ledge, not a
  1.51 m one. With 2.1 m/s2 of air stick the foot sphere needs about 0.4 s over the top to roll round the edge.
- **PS2, BF1** (Kashyyyk Docks' 2.26 m lobe, the same instrument, 16 trials): over the ledge with the apex 0.15 m
  UNDER the top (8 of 8), not with 0.25 m (8 of 8): a 1.912 m jump mounts 2.07 m, not 2.17 m. BF1's air stick is at
  1.3 m/s at once; what is left is the foot sphere on the edge.
- **Port**: 1.75 m in BF2 (margin +0.11; 1.80 m before JARC) and 2.16 m in BF1 (margin -0.25; 2.20 m before). JARC's
  0.26 m taller jump reaches no taller ledge. BF1's is inside the PS2's bracket; BF2's pressed jump gets onto ledges
  0.24-0.31 m taller than the PS2's. The JARC card's acceptance 7 (two Kamino rim stops OVER again) was not met for
  the same reason - and on the PS2's rule those stops stay shut.
A player standing at a crate or a low wall and pressing jump gets on in the port where the PS2 wants a run-up.

## Evidence (`analysis/re-stance-roll.md` §4.3c; `engine/artifacts/lead_tmp/audit-jarc/README.md`)
- PS2: `lead_tmp/audit-jarc/ps2/rim_jump.py ... --shifts` at BF2 Kamino hub 63 (a flat top 2.405 m over the deck);
  `ps2/tools_shift_read.py out_shift bf2_shift|bf2_shiftb|bf2_shiftc` prints apex, margin, advance per trial; BF1 at
  Kashyyyk Docks with `spots_kas2.json` (the `RIM` lines of `ps2/bf1rim_run.log`: end_dy +2.24 = on the lobe). The
  whole jump is raised at its take-off row, so one ledge stands in for lower ones; a write that tears an update shows
  as an apex of 1.2-1.8 m with a 0.48 m kick away and is left out.
- Port: scratch tests `tools/reach_test.rs.txt` and the 1 cm table in `tools/scratch_tests_main.patch`
  (`audit_jarc_reach3`): `reach_audit-jarc-{pre,land}-scratch.txt`, `reach3_main.txt`.

## Acceptance
1. Measure first, same instrument: the running jump in both games (jump 1-3 m before the face; `rim_jump.py`'s
   `run` mode with `--shifts`). Write the thresholds here beside the pressed ones (BF2 +0.35 no / +0.42 yes; BF1
   -0.25 no / -0.15 yes).
2. The pressed-jump table as a committed test in `sim::step`: the tallest ledge mounted inside the PS2's bracket in
   each game (BF2 1.44-1.51 m; BF1 2.07-2.17 m, where the port's 2.16 m already is).
3. `no_jump_passes_a_ledge_taller_than_its_reach` still passes; JWAL's band tests unchanged.
4. The leak census per class, 8 seeds against a null run, the rim probe's Kamino totals beside JARC's.
Declares sim; sweep; arena; leak.

## Notes
- The deck side of the same code (a jump straight up from under a thin deck ends ON it up to 2.60 m / 2.70 m over
  the floor since JARC, 2.35 / 2.40 m before) is card VCOL's ceiling stop; `tools/deck_test.rs.txt` is its check.
- Mutation M7 (d3ff7dc's steep-face exception off) passes every test: give that rule a test here.
- Kamino hub 98 is no place for the instrument: its stored height is the corridor's ceiling (README).
