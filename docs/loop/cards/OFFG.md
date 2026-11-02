---
id: OFFG
title: BF2 Kamino: bots are off the nav graph 47 % of the time and grind along rails and walls at 0.8 m/s - walk back onto the graph before routing (kills 3.3 / min against the PS2's 17.3; rounds take 35-58 min)
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/ai/route.rs,engine/crates/core/src/ai/brain.rs,engine/crates/core/src/ai/steer.rs,engine/crates/core/src/nav.rs,engine/crates/game/examples/round_to_outcome.rs
declares: sim; sweep; arena
needs:
effort:
branch: offg
resume: landed fb216e60 2026-10-02
created: 2026-10-01
first: 3
alpha: yes
---

## Result (2026-10-02, lane 2; `lead_tmp/offg/README.md`, report `reports/20261002_0530_card_OFFG.md`)
- Re-taken on main after BOWL: N 13.6 %, slide 19.8 %, 7/8, 8.0 kills / min. **Step 2: the PS2's own soldiers read
  33.2 % off the graph in the port's `locate`** (`LOCUS_CSV`): the off-graph share was never the defect.
- Step 1's split: 12.7 % of unit time on legs given up before their hub, the 14 worst all BRIDGES - the steering's
  level wall sweep called a bridge's 32 degree end ramp a wall. Fixed: `steer::ramp_clear`.
- Acceptance, BF2 KAM1, 8 seeds x 60,000 ticks, same base: N 6.3 % (bar 15), slide 9.2 % (bar 20), kills / min 13.8
  (bar 10; PS2 17.3), 8/8 finished, median 24,988 ticks; 24 more seeds 24/24. POL1 N 34.6 -> 31.3 %. Recycles on KAM1
  35 -> 7 (0 at the listed KAM1 spots; the other maps' listed spots are not ramps - noted on STARC). `lost`: UTA1
  29 -> 31, BF1 HOT1 0 -> 0, KAM1 1 -> 3.
- NOT met to the letter: "no map's 8-seed finish count drops" - BF2 GEO1 8 -> 6 and BF1 YAV1 8 -> 7 on seeds 1-8;
  over seeds 9-32 they are 22 -> 23 and 24 -> 24 of 24 (noise; README section 4).
- HOLD's table: KAM1 is 8/8 with holds on, so no hold-rule card. What is left on Kamino is contact: card **CONT**.

## Why
The maintainer, 2026-10-01: some maps go really long or never finish. Kamino is the long one, and the maintainer plays Kamino. The INBOX note
of 2026-10-01 14:40 asked for a kill-rate card: this is it. It is contact, not aim.

## Evidence (`lead_tmp/endings/README.md`; main 4603e9e; samples in `lead_tmp/endings/samples/`)
- 8 seeds at 120,000 ticks: BF2 KAM1 rounds take 35, 36, 45, 51, 51, 55, 58 minutes and one never ends; 3.3 kills / min
  (PS2 17.3, re-round-stalls section 4). Every map that ends normally runs 10-37.
- Seed 1, share of alive unit-seconds by `NavGraph::locate`: KAM1 hub 31 / arc 22 / **Nearest 47 %**. POL1 Nearest 32 %,
  HOT1 3 %.
- Steering: KAM1 **slide 42 %** (POL1 26 %, HOT1 12 %). Inside locus Nearest, 64 % of the time is a wall slide.
- Net progress over 10 s while sliding 0.82 m/s, while `fwd` 2.17 (HOT1 1.58 / 4.98).
- So: an enemy within 60 m only 20 % of the time (POL1 56 %), 2.0 AI shots / s (HOT1 11.5), hit rate 0.15 (HOT1 0.22).
- Top slide cells (10 m): (-180, 68, 30), (80, 51, 80), (40, 53, 130), (40, 51, 10), (-80, 67, 200), (-60, 60, 330); the
  15 largest hold 48 % of the slide time. Nearest-hubs 65, 41, 52, 38; hub 100.

## Notes
- STARC (C5 / C6) is the same family on other maps: the POL1 radar door (18, 25, 84) is still the top walking-in-place
  spot on seed 1 (456 unit-s). If one rule covers both, close STARC's part and say so on its card.
- **Memory cap (2026-10-01: a runaway scratch binary took 46 GB and the kernel stopped the whole desktop session's
  editor).** Run any new probe or example binary as
  `systemd-run --user --scope --quiet --collect -p MemoryMax=8G -p MemorySwapMax=0 -p OOMPolicy=kill -- <cmd>` with a
  `timeout`.
- AUDIT of STFAL (2026-10-01): on main 58fa0e3 KAM1's unit 29 stands at `SteerPick::Ledge` at (52, 53, 139) for a whole
  round in seeds 7 and 8 (53,745 / 51,770 ticks). KAM1's sea deaths are 0 on main (8 seeds; lost 15, under 10). At STFAL
  346 died at the east bridge alone (46.7-47.1, 51.5, 186-196), 245 of them pushed off the rim by the unit-separation
  pass, which has no floor check: wherever bots gather at an edge again, that is how they will go over. Numbers and the
  instrument: `lead_tmp/audit-stfal/README.md`.

## Note from RCYC (2026-10-01) - the recycle spots this card owns, and an acceptance line
RCYC's last resort (branch `rcyc`: a bot that wants to walk and makes no net progress for 120 s is recycled) was used 496 times over 8 seeds x 32 maps x 120,000 ticks; 79 spots were recycled on 2 or more seeds. `lead_tmp/rcyc/owners.py` gives this card 17 of them (68 recycles) by a HEURISTIC (off the graph (locus N) and not on a last leg) - look at each spot before trusting it. Re-list: `sweep_seeds.py --seeds 1-8 --ticks 120000`, then seeds.md's `## Recycles`; the run behind this table is `lead_tmp/rcyc/sweep120k_d`.

**Acceptance, added: 0 recycles at these spots (seeds.md `recycled` column and spot list) on this card's maps.**

| map | spot (x, y, z) | recycles | seeds | leg | locus | floor dy | steer | basis |
|---|---|---|---|---|---|---|---|---|
| bf1_bes1 | (-12, 108, -240) | 4 | 4 | 25->2 dest 54 [table] x2 | N25 x3 | +0.0 | slide x3 | off the graph |
| bf1_bes2 | (-246, -33, 90) | 4 | 3 | 83->82 dest 52 [table] x3 | N83 x4 | +17.1 | slide x4 | off the graph |
| bf1_end1 | (168, -3, 0) | 5 | 4 | 54->46 dest 89 [table] x2 | N54 x5 | +0.1 | back x4 | off the graph |
| bf1_kas1 | (30, -27, -210) | 7 | 6 | 107->106 dest 50 [table] x2 | N107 x7 | -8.7 | slide x7 | off the graph |
| bf2_dea1 | (-162, 51, -150) | 6 | 4 | 106->59 dest 76 [table] x5 | N106 x6 | -20.6 | slide x6 | off the graph |
| bf2_dea1 | (-162, 51, -162) | 3 | 3 | 104->60 dest 76 [table] x3 | N104 x3 | -20.6 | slide x2 | off the graph |
| bf2_dea1 | (-156, 51, -156) | 2 | 2 | 104->60 dest 32 [table] x1 | N104 x2 | -21.2 | slide x2 | off the graph |
| bf2_kam1 | (-78, 66, 198) | 5 | 4 | 65->60 dest 100 [table] x4 | N65 x5 | -2.8 | slide x3 | off the graph |
| bf2_kam1 | (78, 51, 84) | 5 | 4 | 41->14 dest 55 [table] x2 | N41 x5 | -0.6 | slide x3 | off the graph |
| bf2_kam1 | (-84, 66, 198) | 5 | 3 | 65->60 dest 100 [table] x1 | N65 x5 | -3.1 | fwd x4 | off the graph |
| bf2_kam1 | (-36, 60, 294) | 3 | 2 | 52->55 dest 100 [table] x2 | N52 x3 | -0.8 | slide x3 | off the graph |
| bf2_kam1 | (-42, 57, 282) | 2 | 2 | 52->68 dest 2 [table] x1 | N52 x2 | -3.1 | slide x2 | off the graph |
| bf2_yav1 | (132, -27, -432) | 3 | 3 | 35->34 dest 4 [table] x1 | N35 x3 | -91.5 | slide x2 | off the graph |
| tat1 | (-66, 15, 78) | 4 | 2 | none x4 | N98 x4 | +7.1 | slide x4 | off the graph |
| tat2 | (126, 0, 60) | 4 | 4 | 26->25 dest 168 [table] x2 | N26 x4 | -4.6 | slide x3 | off the graph |
| tat2 | (102, 0, -12) | 3 | 3 | 14->15 dest 110 [table] x1 | N14 x3 | +0.0 | slide x3 | off the graph |
| tat2 | (96, 0, -12) | 3 | 2 | 14->15 dest 168 [table] x1 | N14 x3 | +0.0 | fan +90 x1 | off the graph |

The census now prints OFFG's numbers per map (`shares <key>:` on stderr with STUCK_CENSUS=1, and seeds.md's last table): BF2 KAM1 on the RCYC branch, 8 seeds x 120,000 ticks: off graph (N) 43.5 %, slide 36.1 %, slide within N 58.6 %, walking in place 6.7 %; POL1 N 33.1 %, DEA1 N 38.9 %, HOT1 N 4.8 %.
Recycles are 92 of Kamino's 1,411 deaths in 8 seeds (6.5 %): judge this card's kill rate and round length with the `recycled` column beside them, and drive it to 0.

- Planner 2026-10-01 22:00: step 3 names `Sim::level_at`. Card **LVLF** (value 5, measure) checks that test on terrain first (71 `lost` deaths on BF2 UTA1, 33 on BF1 HOT1, 23 on BF2 KAM1 over 8 seeds on main ce66fe2). Report the `lost` column for those maps before and after this card's rule.

## Note from BOWL (2026-10-02) - measure this card on the new main; what BOWL left on Kamino's rims
- After BOWL (8 seeds x 60,000 ticks, `lead_tmp/bowl/sw2_v1t/seeds.md`): BF2 KAM1 8/8 (main 1/8), median finish tick
  37,274, kills 1,343 (983), `rim` 0.14 % (27.8 %). Seeds 1-2 at 18,000 ticks: off graph (N) 13 % (25-34 %), slide
  19 % (28-32 %). Re-take this card's 47 % / 42 % before building.
- Left for this card (`RIM_LOG=1`, 151 hull entries in 8 x 10 min): 79 AIRBORNE exits over a rail (33 at (-88, 320),
  31 around (-44, 286)); a walk that meets a rail at under about 30 degrees still steps over (the 6 m edge look lands
  on the hull's first metre) and comes back as lost; looking across the barrier as well closes it but cost GEO1 / MUS1
  a round and DEA1 half its kills (`lead_tmp/bowl/README.md`, variant v1e) - the PS2's per-step corner re-aim is the
  faithful form. A lost bot's climb back took 36 s once (seed 2, from (49, 51, 17)).

## Note from LVLF (2026-10-02)
- `Sim::level_at` (step 3) was measured on every `lost` death (`lead_tmp/lvlf/README.md`): it no longer calls Hoth's trenches off, and BF2 KAM1 was not touched (2 lost deaths in 8 rounds, both units that SPAWNED at cp3's pod nodes (-100.6 / -103.3, 61.3, 317.5), which read Off(8.3) - a hub 78 m away, stopped 74 m short by a barrier, says 'over'. 46 confirmed lost spells start there per 8 rounds; 41 walk out).
- A straight line at a hub is not a walk back: `return_point` preferring hubs a straight floor walk reaches took those pod deaths 2 -> 39 (the line at hub 55 meets the pod's wall; hub 54 is the door). Card RETW has the numbers and the flood to use instead.

## Note from AIRC (2026-10-02)
- A bot just OUTSIDE a rail that `Sim::level_at` calls ON (the first 0.75 m of drop since BOWL) is not lost, so
  `brain::lost_jump` never runs for it: it slides back and forth along the rail's outer face. Seen in a unit-test
  world (the `rim_sim` deck with a 0.9 m rail on its rim, bot started 3 m out on the hull: it walked back to the
  rail, read ON at x -11.5 and slid for the rest of 300 ticks). With air control a standing jump with the stick held
  clears such a rail, so the jump rule could serve any bot whose way is shut by a low wall, not only a lost one.
