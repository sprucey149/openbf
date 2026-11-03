---
id: BESR
title: BF1 Bespin Platforms after HUBP: a bot that reaches hub 35 / 101 / 25 off its centre misses the narrow way out (the ramp 35->4 is 1 m south of the arc's line, a rim barrier on its north) - deaths under the floor 147 -> 234 and recycles 5 -> 29 in 8 rounds
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card HUBP (2026-10-06) made BF1 Bespin Platforms worse while it made every other map better: 8 seeds, deaths under
the floor 147 -> 234 and recycles 5 -> 29, "walking in place" 1.4 -> 3.7 % (BES2 0.4 -> 1.0 %). Not a showcase map,
so HUBP landed with it; this card is the repair. The cause is a narrow way out of a wide hub that the old bots
passed only because they all arrived at the hub's centre from one side.

## Evidence
- `engine/artifacts/lead_tmp/hubp/ab/full_base/seeds.md` against `full_var2/seeds.md` (the landed code: seed 1's
  stdout is byte-identical) - the BES1 rows, and the recycle table: new spots `101->62` (132, 102, 198; 6 recycles
  on 5 seeds), `25->2` (-18 / -12, 108, -240; 6), `35->4` (66, 108, -162); base has `last->122`, `85->84` only.
- Deaths under the floor by 12 m cell, seeds 1-4 (`SWEEP_DEATHS=2`): a new cluster at (60, -156) 24 and (48, -168)
  9 against 0 and 3 on the base; the other clusters are unchanged.
- One bot, seed 1, unit 14, traced on both builds (`blackbox_read --unit 14`; base snapshot
  `ab/bb_base/bf1_bes1_s1.json` at tick 1100, branch `ab/bb/bf1_bes1_s1.json`): route 29 -> 34 -> 35 -> 4, dest 113.
  Leg 35 -> 4 (arc 25, table) is a ramp from the platform at y 107.9 down to hub 4 at 98.7, 39 m west. The base bot
  reaches hub 35 from the south-east at (67.1, -163.9), walks west along z -163.2 and is on the ramp at
  (48.7, 105.6, -162.5). The HUBP bot reaches its own point (69.4, -159.5), starts the leg from (68.0, -160.8),
  walks west along z -160.2 and stops at x 63.6: `steer slide STOP Barrier at 1.00 m`, then
  `STOP Wall at 0.84 m body 116 "bes1_bldg_ramp_32 large ramp1"`; 9 s later it is LOST, walks over the rim and
  dies on the kill floor at (56.2, 82.9, -159.7).
- So the ramp's mouth is about 1 m SOUTH of the arc's own line (hub centre 66.6, -162.7 to 27.5, -160.8), with a rim
  barrier and the ramp's side wall on the line's north side. A bot sent by hub 35's centre from the north
  (measured: HUBP variant 5, the same death at tick 622) fails the same way: the centre line itself only works from
  the south.
- Tried in HUBP and not kept (each 8 seeds x 32 maps, `ab/full_var3..5`): a wall sweep and a barrier test along the
  bot's line to its point, and a walk by the hub's centre before a centre-line leg. BES1 stayed at 178-234 deaths
  under the floor and the other maps lost half of HUBP's gain.

## Acceptance
- 8 seeds, BF1 BES1: deaths under the floor at most the pre-HUBP 147 and recycles at most 5, with HUBP's
  "a teammate within 1.5 m" kept (11.0 %, at most 12 %); BES2's walking in place at most 0.5 %.
- No recycle spot on legs `35->4`, `101->62`, `25->2` on 2 or more seeds.
- The other 31 maps: recycles and deaths under the floor not up against the sweep taken on main before the change.

## Notes
- What would cure it is the mover, not the point: a bot stopped by a rim barrier beside a ramp's mouth should go
  round the barrier's end onto the ramp (cards OBST and MOVR: the committed side, the barrier corner). Check this
  card's three legs again after each of them lands; it may close without code of its own.
- A local cure if it does not: per arc, at load, the floor walked along the arc's line and 1 m to each side - an
  arc whose line has no walkable neighbour on one side is a "narrow mouth", and a leg onto it starts from the side
  the walk passes.
- 2026-10-06 AUDIT HUBP (`lead_tmp/audit-hubp/README.md`): the regression holds over 40 seeds, per round - deaths
  under the floor 19.1 -> 28.9 (Welch t +8.6), recycles 0.65 -> 2.73 (+6.3), kills / min 36.7 -> 30.3 (-7.8), rounds
  18 % longer; all of the 32 maps' rise in floor deaths (812 -> 1,204) is this map. The walk from a bot's feet to
  its point breaks here on open air (a drop, 89 % of refusals; 29 % of them in the 12 m cell at (60, -156), hub
  35's ramp mouth). The feet test only where the centre passes it (card HUBO's second trial) took BES1's recycles
  29 -> 12 and its floor deaths 234 -> 224 over 8 seeds - a lead, not a cure.
- The same family elsewhere, small: BF2 KAS2 "lost" deaths 0 -> 8 in 40 rounds, one spot - a bot at its point of
  hub 38 (r 7.0, centre 120.6, -220.2), 5.2-5.6 m west-south-west of the centre at (116.0, -222.6), on a floor 1.1 m
  under the hub's (inside `HUB_POINT_LEVEL`), slides to y 38.4 on the leg to hub 43 and dies lost at (120, 38.4,
  -226) (seeds 7 and 8, units 11 and 9; `audit-hubp/obs_kas2`). BF2 GEO1 lost 0 -> 5, BF1 YAV1 1 -> 7 (its falls
  38 -> 4). Check them with this card's legs.

- AUDIT OBST 2026-10-07: card OBST (e9ce6db) adds 31 % to this map's deaths under the floor (1,039 -> 1,361 over seeds 9-40, t +4.9; main the same) at spots the parent did not have - the box x -114..-102, z -210..-186 (151 deaths in 32 rounds), (156, 144) 36 - by following a rim BARRIER on its committed side off the platform's end (unit 9 of seed 9, spawned at (-87.7, 108.0, -210.9): `lead_tmp/audit-obst/bb/bes1/`). Card OBSR item 3 owns the cure; re-take this card's own numbers after it. The four largest spots ((24, 96), (-72, -108), (-24, -228), (60, -156): 80-190 deaths each in 32 rounds) are the same on the landing and with each of its rules off (the parent's spots were not logged; its total is 1,039).

- 2026-10-09 (card BSPN, note): with the PS2's spawn draw (a post first, then a node) Bespin's team 0 spawns at cp16 / cp17 for 32 / 30 % of its lives (20 / 16 % before) and at cp15 for 8 % (29 %); kill-floor deaths go 273 -> 382 over seeds 1-8 and 818 -> 1,060 over seeds 9-32, recycles 6 -> 15 and 21 -> 22, team 0's wins 7 of 8 -> 1 of 8 and 22 of 24 -> 15 of 24. Runs: `lead_tmp/bspn/sweep_base2`, `sweep_fin`, `conf_base2`, `conf_new2` (per-post shares: `lead_tmp/bspn/tools/shares.py <base> <new> --map bf1_bes1`). Re-take this card's "before" numbers on main after BSPN.

## Planner 2026-10-12 11:26 - no longer owed by BSPN's approval; re-measure before building; waits for MOUTH
- **This card's numbers are older than the corner search.** Its 147 -> 234 deaths under the floor (8 rounds) and BSPN's refused
  `world_death_pct@bf1_bes1` 14.4 -> 18.9 % were measured before OBST2's 7dfb448 (10-09 17:02: 858 -> 124 deaths under the floor
  in 24 rounds). The fidelity baseline re-taken today on main 19708fa (BSPN's spawn rule on) reads Bespin Platforms' world
  deaths at 3.61 / 3.13 % of deaths (seeds 1-8 / 9-16; 2.96 / 2.18 on main 8ad30f4 before BSPN; tolerance 3.8) -
  `lead_tmp/fidelity_rebase_20261012b/`. So I took `world_death_pct@bf1_bes1` and this card out of `gates.toml [approvals.BSPN]`
  (it now owes DAGS only). Undo: the comment in that block says how.
- **deps += MOUTH** (`first: 1`): it measures Bespin Platforms' stalls and falls leg by leg (316 of 394 stalls at `122->7`; 2 of
  17 traced falls on this card's leg 35->4) and builds the general rule for entering a narrow leg. Stays `defer: post-alpha`.
- **Before anyone builds this:** re-take step 1 on today's main (how many bots still miss the ramp 35->4 from hubs 35 / 101 / 25);
  if MOUTH's table shows nothing left on that leg, close the card.

## Card MOUTH, 2026-10-12 (measure) - what of this card MOUTH's build takes, and what it leaves
- Deaths under the kill floor, 8 rounds of 5 minutes (`SWEEP_DEATHS=1`, seeds 1-8): main 19708fa 44, fe043b4 46; with
  MOUTH's rule (the leg's line from the last corner passed, a scratch switch) 8 and 12 - down in all 16 paired rounds.
  By the faller's last leg in the snapshots: `58->59` 5, `35->4` 3, `51->61` 3, `52->50` 2, `12->0` 2, `38->41` 2 -> `35->4` 1.
- Left after it: `35->4` (15 stalls in 70 passes, unchanged), `101->62` / `102->101` (13 in 22 passes), and unit 8's
  loop at the round building (RLOOP / SPNC). The PS2 on the same sites: 0 stalls in 50 and in 34 passes
  (`lead_tmp/mouth/out/bes1_sites.md`). Re-take this card's bars once MOUTH has landed.

## Note from MOUTH's build (2026-10-12 13:31, lane 1; main 273a441)
- BF1 Bespin Platforms after MOUTH: deaths under the kill floor 46 -> 7 in 8 rounds of 5 minutes (seeds 1-8,
  `lead_tmp/mouth/build/deaths/`), and over whole rounds 156 -> 39 in 24 seeds (`build/sw/j_00`, `j_MH`; 58 -> 11 in
  seeds 1-8 against this card's bar of 147). Recycles 28 -> 28. Re-measure this card's legs (`35->4`, `101->62`,
  `25->2`) on today's main before any work: `58->59` and `51->61` were MOUTH's defect.
