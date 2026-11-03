---
id: OBSV
title: Bots go round parked vehicles and turrets by their footprints (bot design 4.7 item 2, split off OBST2): a 2.4 x 4.8 m hull costs a bot over 1 s of sliding at 44 of 252 placements and com_weap_gunturret is the small body with the most contact time - the footprint as a rectangle passed by its corners on the committed side
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Split off OBST2 by the planner (2026-10-12 14:22). OBST2 landed three times (7dfb448 the corner search, 600fac7 the black
box's row, fe043b4 the cost cuts) and its one design item not built is this: bot design 4.7 item 2. Today a bot meets a
parked vehicle or a turret as any other body - a hull over 3 m wide is gone round by OBST's rules 1-3, and
`com_weap_gunturret` is the small body with the most contact time.

## Evidence
- OBST2's card, "What is left" 2, and its `resume:` (2): the scene is a 2.4 x 4.8 m box at 252 placements; 44 of them
  cost a bot over 1 s of sliding on main fe043b4.
- `engine/artifacts/lead_tmp/obst2/README.md` (the tools), `lead_tmp/obst/ab/final/` (the body census).

## Notes
- Not this card, and where each went: B13's cost bar (0.187 ms a tick for the bar 0.123) is on PERF; velocity-space
  avoidance of soldiers (design item 3) is not built - the corridor scene passes on main, so it is a looks question;
  B10's residue and variant h1 (30 fewer recycles over 24 seeds, worse in the maintainer's scene) were measured and not taken.
## Session 2026-10-12 (lane 1, 14:42 - 17:00): built on branch `obsv` 10143ae, ci green, NOT landed - the fidelity gate refused all-map recycles on seeds 1-8 (NOISE by three blocks), card blocked for the planner
Everything is indexed in `engine/artifacts/lead_tmp/obsv/README.md`; report `loop/reports/20261012_1653_card_OBSV.md`.

**1. Re-measured on main 17ae28e** (8 seeds x 32 maps x 10 min, `census/main_i`): the 252 placements 42 over 1 s, 67 with
a change of side (as a hull 41 / 67, 251 touched). Contact seconds: vehicle hull 14,462 (0.70 % of walking time; at a
vehicle under 1.5 m/s 8,791, at an empty one 5,552, boarders 152), top five map classes bes1_bldg_main02 4,524,
geo_bldg_bunker_1 4,320, pol1_bldg_radar_bridge 2,914, end_bldg_ewok_ramp_26 2,458, kas2_bldg_platform_roof 2,097.
`com_weap_gunturret` 688 s - 677 of them ONE turret on BF1 Endor, and not an avoidance question: a detour corner of the
leg 18 -> 0 lies in its middle (card OBSV2).

**2. The rule, behind switches in one binary** (`obsv_scratch_knobs_k9.patch`, null = main byte for byte). Taken: bit 1
(a standing vehicle's footprint passed by its corner on one side) + bit 16 (a walk point under a standing vehicle is
reached at its side). Not taken: empty vehicles only, leave a footprint by its nearest side, the walk point in a map
body (OBSV2).

**3. Bars** (the branch = `bin/k9 OBSV=17`, equal to the committed code over whole rounds of seed 42):
| bar | main | branch | |
|---|---|---|---|
| 252 placements over 1 s / touched / changed side | 41 / 251 / 67 | 0 / 0 / 0 | MET (the test asserts it) |
| hull contact, all / standing / empty vehicles (s) | 14,462 / 8,791 / 5,552 | 7,293 / 2,414 / 1,741 | - |
| `com_weap_gunturret` halved | 688 s | 502 s | NOT MET here: card OBSV2 (13-26 s with its rule) |
| 24 seeds x 32 maps: rounds | 768 / 768 | 768 / 768 | met |
| recycles (paired t) | 145 | 150 (+0.3) | met |
| lost / floor deaths | 58 / 49 | 59 / 40 | met |
| the maintainer's Kamino scene (two seed blocks) | 5 of 411, 5 of 403 | the same (no vehicle on the map: rows byte-equal) | met |
| ms a tick, four B13 maps, 3,000 ticks, 8 seeds, JOBS=1 | 0.316 (its twin x 1.013) | 0.313 (x 0.996, sd 0.045) | met |
Per map over 72 more seeds: Mos Eisley recycles 168 -> 153; **BF1 Endor 37 -> 61**, 20 of them one speeder-bike driver
in the round's opening (card HJAM) - a real per-map cost of the rule, not in the driver's code path, not shown on main.

**4. Tests**: six for rule 5 and its arrival half, 18 mutations each fail one (`mut/*.txt`). **5.** the cylinder ray
against an inside-test oracle: 7,500 rays of five kinds, 6 mutations fail it (one fails only it).

**The gate** (`gates_fid/`): `recycled (all maps) 30.00 -> 58.00` on seeds 1-8, refused. Sorted NOISE
(`fidelity_noise.txt`): by 8-seed block main reads 40 / 48 / 57 and the branch 58 / 49 / 43 (24 seeds 145 -> 150, paired
t +0.26); the baseline's 30 is main d096ae7's, today's main reads 40 on the same seeds; the block's rise is Mos Eisley
(8 -> 19), which over seeds 9-80 is 168 -> 153. I did not approve it: the planner re-takes the baseline and judges.

## Planner 2026-10-12 17:05 - the refusal judged: NOISE on the gate's number, and the card waits for HJAM
- Baseline re-taken on main 5fbbbd4 (code = 17ae28e), both blocks: all-map recycles 40 / 48 - the session's own reading
  of main, reproduced by the gate's run (`lead_tmp/fidelity_rebase_20261012d/gates/fidelity/`). The branch's 58 on seeds
  1-8 is still 18 over (tolerance 12) and the gate has no second block for an all-map value, so `parked` would be
  refused again by the same binary. NOISE stands on the evidence (three blocks 40 / 48 / 57 against 58 / 49 / 43; 24
  seeds paired t +0.26); the gate's rule for sums is card FIDA (waits on the maintainer).
- Endor's bike is CAUSED in the prompt's sense (only the branch shows the recycle; main's count at that spot is 0), so
  no approval is mine to give. Taken: `status=todo`, `deps += HJAM`. HJAM is value 5 now.
- Not judged: whether the maintainer would rather have the rule now with the Endor cost (STATUS question 38).
- 2026-10-13 09:25 (card HJAM, lane 2): the 20 / 21 recycles of unit 5 at the Rebel spawn are traced
  (`lead_tmp/hjam/README.md`): the bike circles a detour corner of its leg until its window fails (HJAM's, built: branch
  `hjam` 7af3660), comes back at 31 m/s INTO the hollow trunk `end_prop_tree04` and the driver gets out inside it (card
  VMESH's fault). With HJAM's fix under this card's rule (main 5f8f569 + 10143ae + the fix): 0 of 72 seeds there, BF1 Endor
  recycles 60 against 63 for the fix alone, 24 seeds x 32 maps recycles 148 against 139 (paired by seed +0.38 a seed, t +0.4), 768 / 768.
- 2026-10-15 planner: `deps:` HJAM -> VMESH. HJAM waits on the maintainer (question 48); its report (`reports/20261013_0925_card_HJAM.md`, For the maintainer 2)
  shows OBSV's 21 Endor opening recycles are VMESH's trunk door reached through HJAM's circling - it needs HJAM OR VMESH, not both.
  Undo: `lead_tmp/planner_1015/OBSV.before.md`.

## Session 2026-10-15 (lane 2, 14:11 - 15:15): rebased on main b8a634d (VMESH on main), re-measured there, LANDED f02bce2
Rebased (shape.rs: VMESH's `watertight` and this card's `ground_extent` both kept; CRATES.md counts), ci green. On main
b8a634d against the rebased branch (`lead_tmp/obsv/README.md`, session 2026-10-15):
- fidelity gate alone: PASS; recycled (all maps, seeds 1-8) 40 (VMESH's own landing run read 54), noprog 13 -> 10.
- BF1 Endor seeds 9-80: recycles 0 / 0 (main / branch) - the bike's circling at the Rebel spawn is gone with VMESH.
- 72 seeds x 32 maps (1-24, then 25-72 to confirm): rounds won 2,303 -> 2,304 of 2,304; recycles 382 -> 377 (t -0.2);
  floor 184 -> 187 (t +0.2); **lost 182 -> 200** (seeds 1-24 54 -> 66, t +3.7; seeds 25-72 128 -> 134, t +0.8). Half of
  it is ONE main fault more used: BF2 Geonosis's cave mouth `geo_prop_cave1` (70.9, -72.1, 117.7), 21 -> 30, the same
  trace on both builds (leg 37 -> 36, no vehicle within 77 m; note on card RETW); the rest scattered (1-2 a spot) on
  vehicle maps, the first one traced (BF1 Yavin seed 11) is a plain `travel` with no vehicle within 30 m.
- BF2 Kamino (the maintainer's scene): rows hash-equal on seeds 1-8. Cost a tick, the four B13 maps, JOBS=1, best of two loaded runs
  per seed: branch 0.490 ms, main 0.529, main's twin 0.511 (x1.009 against the better of main and twin; the twin x1.055).
- Landing gates: only the declared sweep moved (22 vehicle maps' seed-42 rows), 0 goldens, every other gate PASS.
