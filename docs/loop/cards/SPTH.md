---
id: SPTH
title: BF2 Kamino: cp2's bots spawn on the wrong path - the conquest script moves the post's SpawnPath to cp2_spawn at run time (12 nodes) and the port keeps the world data's path33 (10 nodes, 3-10 m away): PS2 60 of 60 starts at cp2_spawn nodes, port 249 of 250 at path33's. Plus the three parts of the spawn draw the BSPN audit found no test for
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Found by the audit of card BSPN (2026-10-12, `loop/audits.md`), which checked "a bot spawns as the PS2's does" against PS2 captures. The maintainer plays BF2 Kamino; at post cp2 the port's bots appear at ten spots the PS2 never uses, 3 to 10 m from where the PS2's appear.

## Evidence
- **The PS2** (three independent 10 Hz captures of BF2 Kamino conquest, `lead_tmp/mouth/ps2/bf2_kam1_m1..m3_units.csv`): 60 of 60 AI life starts around cp2 are within 0.2 m of a node of the path `cp2_spawn` (12 nodes); 0 are within 1.5 m of a node of `Path33`.
- **The port** (main d096ae7, 8 seeds; the landing 2d1fb60, 8 seeds): 249 of 250 and 232 of 234 starts around cp2 are at `Path33` nodes, 0 at `cp2_spawn` nodes. Tool, both sides in one run: `python3 engine/artifacts/lead_tmp/audit-bspn/tools/kam_cp2_starts.py <sweep dir with botq>..`.
- **Why**: the world data's conquest cp2 (`com_bldg_controlzone` at (108.6, 52.3, 157.7), CaptureRegion `cp2_capture`) authors SpawnPath `path33`. The conquest scripts `kam1c_con` and `kam1g_con` hold the string constants `SetProperty` ... `spawnpath`, `cp2_spawn`, `captureregion`, `cp2_capture` in that order [D: read with `strings`, the call itself not decoded] - the script re-points the post at run time. The port's mission data carries only the `Team` property of a post (`game/src/mission.rs`, `cps`), so the override is dropped.
- **Census** (`strings` over the 128 BF2 mission scripts): 20 hold a `spawnpath` constant; of the 33 conquest scripts only Kamino's two. BF1's mission data has no `SetProperty` at all. So in conquest this is one post on one map, both eras; the campaign scripts use the key freely (not in scope).
- The three unrefereed parts of BSPN's draw (the audit's mutations; each passes every test today; `python3 engine/artifacts/lead_tmp/audit-bspn/tools/mk_mut.py <ID>` makes the mutated file and prints the run command):
  - **A7** step 0 does not ask for a post when the respawn timer runs out (the draw is made only at the spawn): none of the four spawn tests goes through `Sim::step`.
  - **A8** a command walker's post weighs six times a fixed post's: the only test with a walker has it as the single post, where no draw is made.
  - **G1 / G2** the world data's `AISpawnWeight` never reaches the sim's post (`walk.rs`) / is read under a wrong property hash (`layout.rs`): no test in `game` reads the key. (No conquest post authors it, so nothing moves today; campaign would.)

## Acceptance
- `kam_cp2_starts.py` on an 8-seed sweep of the branch: at least 95 % of the port's starts around cp2 within 1.5 m of a `cp2_spawn` node (today 0 of 250), and 0 at a `Path33` node. The PS2 column is in the tool's own output.
- `gen_missions.py` decodes every `SetProperty(<post>, <key>, <value>)` of a conquest script (not only `Team`); the report lists per map the keys found and which the port applies (SpawnPath here; CaptureRegion is equal to the world data's on Kamino - say so with the two values).
- Tests: A7, A8, G1 and G2 each fail a test (run the four through `mut_run.sh`; paste the four `caught_by` lines).
- The sweep, arena and leak baselines move on BF2 Kamino only (declared); the `spawnrule` referee still reads the post-first rule (`lead_tmp/bspn/tools/rule.py`: chi2 against the PS2 rule under about 30 at 17 dof, against the flat list over 400).

## Notes
- cp2's node count changes from 10 to 12, so its share under the flat opening wave (card OPNW) moves a little; the respawn draw is by post and does not care.
- The PS2 captures also show constant offsets between a start and the nearest baked node at cp1 (2.6 m), cp5 (1.7 m) and cp6 (0.9 m) on Kamino, 0.0 m at cp3 and cp4 (`lead_tmp/audit-bspn/wf/K4/out2.txt`). Not traced: check them with the same tool once cp2 is right (the PS2 settles a spawn by a ray up 10 m and down 20 m from the node, re-command-walker-post 4).
- The mutation worktree and its target (`openbf-wt/audit-bspn-mut-scratch`, `lead_tmp/audit-bspn-mut_target`) may be gone by the time this is built: `tools/setup_mut.sh` remakes them at its pinned commit - re-pin it to main first.
