---
id: HJAM
title: BF1 Endor: a speeder bike's driver circles one spot at the Rebel spawn until it is recycled - the opening's three bikes jostle at (-185, 320) on main too (each turns back twice, stuck 60 at ticks 405-495) and with card OBSV's footprint rule one of them stays there in 20 of 72 seeds (0 on main): trace the driver's circling on open ground, name the cause in hover_drive, fix it
milestone: M3
value: 5
phase: build
status: done
deps: WCREW
files: engine/crates/core/src/ai/hover_drive.rs,engine/crates/core/src/ai/route.rs
declares: sim; sweep; arena
needs:
effort: high
branch: hjam
resume: landed 37170e27 2026-10-16
created: 2026-10-12
alpha: yes
---

## Why
Filed by card OBSV (2026-10-12). OBSV's rule (a bot on foot passes a standing vehicle by its footprint) makes two Rebel
bots walk round a parked speeder in the first second of BF1 Endor. That shifts the three bikes that leave the spawn
together (vehicles 6, 7, 11; drivers units 4, 6, 5), and bike 11 then circles a 4 x 5 m box at (-178..-182, 322..327)
- legs 3->4, -1->4, 4->5 in turn, `stuck 60 / 120`, `stop open`, nothing within 12 m, its hull losing health - until the
stall watch recycles its driver at tick 4693-4788. The drivers do not ask the avoidance (`ride.rs` -> `hover_drive::drive`).

## Evidence (all under engine/artifacts/lead_tmp/obsv/)
- `fidelity_noise.txt`, last lines: BF1 Endor seeds 9-80, recycles 37 -> 61; 20 of the 61 are unit 5 at that spot, main 0
  there (`dth/e0_*.err` main, `dth/e17_*.err` the rule: every recycle with its spot, SWEEP_DEATHS=2).
- `bb/e/bf1_end1_s15.json` (MAIN's rules, seed 15, tick 520) and `bb/d/` (the rule): bike 11 every 45 ticks with what is
  near it - on main it is at x -177.6, -186.1, -176.2 at ticks 180 / 225 / 270 (turned back twice among bikes 6 and 7)
  and `stuck 60` at ticks 405-495 at (-158, 313); `bb/b/` (the rule, tick 900): the trail going nowhere.
- Not checked: whether main's own Endor recycles at (-160, 296) (2 in 32 rounds) are drivers too.

## Measured 2026-10-13 (lane 2; evidence `engine/artifacts/lead_tmp/hjam/README.md`, report `reports/20261013_0925_card_HJAM.md`)
Re-measured on main 5f8f569 with OBSV's commit on top: BF1 Endor seeds 9-80, recycles 31 (main) and 69, 21 of them unit 5
at (-180, 324), ticks 4693-4788. The card's reading was half right. Seed 15, unit 5, every tick (`tr/on.err`):
1. **The circling is a detour corner** (`core/src/ai/route.rs` `advance_route`: a corner is passed inside `CORNER_ARRIVE`,
   1 m, whoever asks; called from `hover_drive.rs` `drive`). Leg 3 -> 4 has two corners; the bike passes the first,
   (-186.2, 320.3), 3.0 m wide at 36 m/s and then turns round it at full stick, 1.3-2 m from it, ticks 140-440. Second
   fault, same place (`hover_drive.rs` `drive`, the `ai.leg` test): a corner takes the hover's own height, so the node
   "changed" every tick and the segment was begun anew from the corner to itself - no point ahead on a line, and the stuck
   test's 5 s clock never ran. On main, 30 maps x 8 seeds: 16 % of all hover driving time is within 8 m of a corner (BF1
   Endor 43 %, BF1 Yavin 50 %), and 1,153 of the drivers' 2,303 failed progress windows are at one.
2. **The recycle is card VMESH's fault, not the driver's.** After the failed window the re-plan sends the bike back by hub 3
   and it returns at 31 m/s INTO `end_prop_tree04 @ (-180.4, -0.1, 325.4)` (a hollow trunk 8.7 m across, 29 triangles, open
   below; inside by ray parity from tick 540, `tr/tree04_parity.txt`). The unstick manoeuvre does not move it, at
   `stuck_run 180` the driver gets out inside and walks the trunk's inner wall (40 m walked, 0.2 m net in 10 s) until the
   stall watch takes him. On MAIN 28 of the 31 recycles are the same thing at other trunks (a driver recycled within 10 m
   of where he bailed, 19 of 72 rounds; `tools/exdrv.py`).

Built: branch `hjam` 7af3660 - a hover's driver reaches a corner at its node radius (ForwardSpeed x 0.5, re-hover-ai 5)
and its segment to a corner is begun once; two tests, each failing under its own mutation only. ci green. Not gated.

| BF1 Endor, seeds 9-80 | recycles | a driver in the opening at the Rebel spawn | an ex-driver where he bailed | driver-s within 8 m of a corner | failed windows at a corner | crashes a ride (PS2 0.28) |
|---|---|---|---|---|---|---|
| main | 31 | 0 | 28 | 35,941 of 88,719 | 4,563 | 0.25 |
| the fix | 63 | 0 | 59 | 117 of 57,568 | 131 | 0.28 |
| main + OBSV | 69 | 21 | 66 | 38,753 of 93,307 | 4,964 | 0.24 |
| the fix + OBSV | 60 | 0 | 59 | 116 of 56,778 | 88 | 0.28 |

24 seeds x 32 maps, paired by seed: rounds 768 / 768 in all eight runs; recycles main 142 -> fix 139 (t -0.1), with OBSV
151 -> 148 (t -0.3); lost 55 -> 54; vehicle deaths 1,339 -> 1,589 (BF1 Endor 289 -> 440, the Dune Sea 30 -> 69). The Dune
Sea's death-region deaths (0 -> 27 on seeds 1-24) are a skiff in the Sarlacc pit and main's too: 96 more seeds, 63 deaths
in 20 rounds on main, 62 in 18 with the fix. Two other corner rules measured and not taken (beyond the corner's plane;
early only when the line to the next point crosses no barrier): no all-map difference, 28 to 42 times more time at corners.

**Acceptance, as it stands:** traced, cause named with file and line - done. A test that fails without the fix - done (two).
With OBSV on: no driver recycled in the opening (21 -> 0) - done; all maps not worse - done. **BF1 Endor's recycles at or
under main's - NOT met (31 -> 63, with OBSV 60), and no driver fix can meet it:** a bike that no longer circles rides more
(3,779 -> 4,357 rides) and more of them end in a trunk. That bar measures VMESH. For the planner: land this before VMESH
(the circling goes, Endor's trunk recycles double until VMESH) or after it (then the bar is re-measured). My view: before -
Endor is not a showcase map, the circling is in the first five seconds at the Rebel spawn, and OBSV then costs nothing on
this map (60 against 63). OBSV's own 21 are VMESH's third door, so OBSV waits on VMESH or on this card, not on both.

## Gated 2026-10-15 (lane 1; report `reports/20261015_1028_resume_HJAM.md`, evidence `lead_tmp/hjam/fidsort.md`)
Branch `hjam` rebased on main 1cf5c8b -> **b7b2062** (CRATES.md count conflict only), ci green (`lead_tmp/hjam/ci3.log`).
`gates.py --auto` (`lead_tmp/hjam/gates/SUMMARY.md`): ci MOVED 1474 -> 1476 tests (the two new), gpu / sim / simhash / arena x2 /
leak / protect / map PASS, sweep MOVED (15 rows, the hover maps; declared). **Fidelity FAIL on one undeclared value:
`stuck_max_med` (all maps) 166.88 -> 185.62** (limit 181.88). Sorted **NOISE**: six 8-seed blocks x 32 maps on each binary
(main = `bin/main2`, branch = `bin/fix2`): main 166.9 / 183.8 / 181.9 / 161.3 / 183.8 / 174.4 (two of main's own over the
limit; the same six values as HVYW's main table, so VTURD moved no sim), branch 185.6 / 166.9 / 172.5 / 172.5 / 185.6 / 176.3;
paired by block +1.25, sd 13.0, t +0.24. Same 48 seeds: recycled 287 -> 282, noprog 102 -> 95, finished 1,536 / 1,536 both.
`world_death_pct` (all maps, not refused, tol 0.4): 1.15 -> 1.27 in every block - all of it riders of destroyed vehicles
(2,657 -> 3,119); world deaths without riders 704 -> 723. The gate's noise row `world_death_pct@bf1_end1` (5.13 -> 9.11 / 8.95)
over 96 fresh seeds (17-112): 6.62 -> 8.37 (per-map tol 3.8): riders 1,271 -> 1,589 (bikes that ride instead of circling,
crashing a ride at the PS2's 0.28) and recycled ("stalled") 40 -> 82 - the trunk riders of card VMESH, the card's own unmet bar.
- 2026-10-15 10:45 planner (lane 1): the NOISE table read (`lead_tmp/hjam/fidsort.md`); not approved by me - an all-map value is the maintainer's
  (his HVYW yes named that card), so `needs:` = STATUS question 48. Order meanwhile: VMESH first; OBSV now depends on VMESH instead of
  HJAM (HJAM's report: OBSV needs this card OR VMESH). ON THE MAINTAINER'S YES: `[approvals.HJAM] fidelity = "stuck_max_med@agg"`, re-measure the
  Endor bar on top of VMESH if it has landed (else land with the bar waived as the maintainer says), `status=parked`. Undo: `lead_tmp/planner_1015/HJAM.before.md`.

## Notes

- 2026-10-15 14:10 lane 2 (card VMESH LANDED b8a634d): your Endor bar's cause is on main now. With VMESH, BF1 Endor seeds 1-288: all recycles 176 -> 1 (main 662e152), riders no longer put down inside trunks or the boulder (exits inside a body 0 on 32 maps x 8 seeds), a vehicle a driver bailed out of as stuck is not re-boarded until it moves 2 m. Re-measure HJAM's Endor bar on the rebased branch (`lead_tmp/vmesh/tools/run.sh`, `lead_tmp/hjam/tools/exdrv.py`); expect the 40 -> 82 to be gone. The trunks are OPEN shells: VMESH's keep-out (watertight bodies only) does not stop a bike entering one - only the exit and the stuck mark cover them.

## Land attempt 2026-10-16 02:20-02:54 (lane 1; report `reports/20261016_0255_land_HJAM.md`)
- Rebase on 5e25eac (WCREW) conflicted on CRATES.md counts only; resolved -> **b6f4904**. land.py phase 1: ci 1559/0/46,
  sim / simhash / arena x2 / leak / protect / map PASS, sweep MOVED (declared), **fidelity FAIL: `recycled` (all maps) 19 -> 40**
  (tol 12). Nothing merged.
- Sorted **NOISE** (`lead_tmp/hjam/land3/tally.txt`, `ab48.sh`): seeds 1-48 x 32 maps, main 5e25eac (`bin3/main_5e25eac`, passes
  the seed-42 check) 214 vs branch 224; by 8-seed block main 20/36/34/36/33/55, branch 40/31/31/42/34/46, paired +1.67, sd 10.4,
  t +0.39; rounds finished 1535 / 1536. Main's own binary is over the limit (31) in 5 of 6 blocks; the baseline's 19 is its lowest
  block. Maps with no hover are identical per map (bf1_bes1 45/45, bf1_geo1 8/8, ...). The gate re-checks per-map values on seeds
  9-16 but never the all-map value (gates.py fidelity_confirm, "agg stays"), so a re-taken baseline cannot help: the maintainer's call (Q37 FIDA).
- **Endor bar re-measured on top of VMESH:** BF1 Endor seeds 9-80, recycles main 0, branch 0; drivers in the opening 0 / 0
  (`dth/l3_main`, `dth/l3_fix`). The card's acceptance is now met in full.

## Landed 2026-10-16 10:02 (lane 1; report `reports/20261016_1002_land_HJAM.md`)
- Rebased b6f4904 -> 34345c9 (CRATES.md counts only), main 37170e2. Gates: only the approved recycled@agg (20 -> 35) and the declared sweep moved.
- Re-measured: BF1 Endor seeds 9-80, recycles 0 on the landed binary, 0 on main before (`lead_tmp/hjam/dth/l4_fix`, `l4_main`).
- Planner: remove `[approvals.HJAM]` from gates.toml.
