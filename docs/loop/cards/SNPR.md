---
id: SNPR
title: Snipers with a purpose: a scoped-rifle bot picks the Snipe hint or high ground with the longest clear lane onto enemy ground, holds it while enemies are in the lane, relocates after 20 quiet seconds, and takes its pistol when an enemy is inside the rifle's min range or within 25 m and closing (the maintainer's INBOX 2026-10-12 16:20, ask 2; deviations approved)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer's wish (INBOX 2026-10-12 16:20, ask 2 - read it first; under `## Done` now): snipers with a purpose. A
sniper-class bot (scoped rifle) picks a spot with a long clear lane, holds it while enemies are in the lane, fires at
range, takes its pistol when an enemy is close, and relocates when the lane goes quiet. Deviations from the PS2 are
approved (his 16:00 note).

## Evidence
- Code (main 5fbbbd4): a scoped weapon taps once per 2-3 s with the PS2's distance-table spread
  (`ai/gunner_foot.rs` 103-116, 550-600; `analysis/re-ai-sniper.md`); a hold may take a Snipe hint
  (`ai/behave.rs` 614-700: the nearest authored Cover / Snipe hint within `hint_r`), but no bot SEEKS a sniping spot.
- **Measured the same afternoon by card CLSE4** (`analysis/re-soldier-ai.md` 20, `lead_tmp/clse4/`): on BF2 Kamino the
  PS2's sniper holds the rifle 21.5 % of the AI foot time and aims at a soldier 0.4 % of that - 99 % in the snipe
  stand, still, 23 shots in 3,051 unit-seconds. The port's sniper is an aimer 10.2 % of its time and deals a third of
  the map's far-damage excess over the PS2. So the PS2's sniper is a statue that hardly shoots, and the port's is a
  rifleman with a scope. The maintainer asks for neither: a spot chosen for its lane, held while it pays, left when it does not.
- `arsenal::choose` scores by each weapon's `ai_range` band (line 161): whether a scoped rifle's min range hands over
  to the pistol at 15-25 m today is NOT checked - step 1.

## Notes
- 2026-10-12 16:58 planner: filed from the maintainer's note. `deps: CLSE4` because CLSE4's build puts the trigger's `MaxRange`
  test and the PS2's burst keys into `gunner_foot.rs` and re-takes the far-fire table this card is judged beside; it
  does not wait for TPOS / TEAM (it uses the hints and hubs that exist; TPOS's table replaces the scoring's input
  later). CLSE5's sniper question (the PS2's long stand) is answered by the maintainer's note: he wants a purposeful sniper, so
  CLSE5 only measures.
- Keep `gunner_foot.rs` edits out of this card if CLSE4 or CLSE2B is in flight (both list it).
- 2026-10-15 19:23 (lane 1, build session stopped by STOP at 19:14): **step 1's census on main 70b9c47** (8 seeds x 32
  maps, CLSE4's observer columns, `lead_tmp/snpr/census_main8.txt`, `tools/census.py`): 10,475 sniper lives, 469,582
  foot-seconds; at a Snipe hint 0.5 % of foot time (BF1 0.3, BF2 0.9); travel 56 %, fight 36 %. Weapon in hand by
  target distance, pistol share: <15 m 79.4 % (bar: over 80), 15-25 m 52.7 %, 25-40 m 14.3 %, >40 m 3.5 % (BF2 7.0 %:
  look at it - a reloading rifle is passed over). Rifle shots beyond 30 m 84.5 % (33,195 of 39,304; >60 m: 9 -
  the sight is 49 m standing at Normal). Deaths: nearest enemy median 21.2 m, 35 % inside 15 m. The AI ranges:
  BF2 rifle 8/64/128, pistol 0/16/24 (commando 32); BF1 rifle 20-32/64/128, pistol 0/16/24-32 - so `choose` already
  hands over near 15 m by band (BF1) and score (BF2); the rule is about the 15-40 m middle and the switch back.
  OPEN QUESTION for the planner: a sniper's sight is perception's `view_distance` (no scope term, 49 m at Normal), so
  a lane can be no longer than that and "fires at range" means 30-49 m; a scope multiplier is `perception.rs`, outside
  this card - the planner / the maintainer decide whether a sniper on its spot sees farther (a named deviation; it adds far fire,
  which CLSE4 measured as already over the PS2's).
- 2026-10-15 22:00 (lane 2, build): **built, 8cd4704 on branch snpr** (rules always on; the switch was measured, then
  removed). Census, 8 seeds x 32 maps, CLSE4's observer, main 70b9c47 against the branch (`lead_tmp/snpr/census_*.txt`;
  null `SNPR=0` = main's rows on all 8 seeds, cmp): a sniper's foot time at a picked spot 1.1 % (main, the branch's
  spots as a control) -> 9.1 %; at a Snipe hint 0.5 -> 5.8 %; 30.7 % of sniper lives reach a spot (19.6 % of their
  foot time on it, first there after a median 15 s; median life 37.5 s on foot - the life is the bound); pick lane
  median 43.5 m; pistol in hand inside 15 m 79.4 -> 81.3 % (bar 80), 15-25 m 52.7 -> 59.8 %; rifle shots beyond 30 m
  84.5 -> 86.9 %; contact20 (all maps) 12.00 -> 11.89 %, BF2 Kashyyyk 12.44 -> 10.31. Reach swept: 120 m 7.0 %, 60 m
  9.1 %, 40 m 8.7 % -> 60. Found and fixed on the way: (a) thinks on one tick read the old state, so snipers that
  spawn together took ONE spot (log: two at (40.8, 3.9, 109.1) at t=0) - the higher index now yields; (b) the first
  pistol rule forced the rifle when its rules did not ask for the pistol, so the pistol inside 15 m FELL 79.4 -> 74.0 %
  - it now only draws / keeps the pistol; (c) a pistol kept to 40 m is silent past its 24 m MaxRange (gunner_foot's
  trigger) - kept to its own reach. Mutations: 12, each fails a test (`lead_tmp/snpr/mut/`).
- 2026-10-15 23:45 (lane 2): gates refused on fidelity (gates1 on 8cd4704: recycled 60; gates2 on 2909492: recycled
  56, idle20 3.20 -> 4.02, stand20 0.08 -> 0.45). Sorted (report 20261015_2120): stand20 / idle20 = the hold the maintainer asked
  for (14,091 of 15,339 s are snipers on a picked spot; labelled `Behaviour::Overwatch` in 5f258cd, `bot_quality.py`
  STANDS on the branch - the land gate reads main's copy). recycled: NOT noise (24 seeds, one base: main 134, spot
  rule 157, pistol rule 185, final 161); NOT exposed: BF2 Kashyyyk's pen (RLOOP's, body 222) gets the same entries
  (team 0 -> cp2 165 / 165) and traps more of them (0.6 -> 6.1 %), no sniper near, non-sniper code unchanged; Bespin
  1's 14 -> 28 comes with the pistol rule but is one comparison (the spot rule never acts there). 2909492 fixed one real
  defect on the way (the pistol drawn at 24-32 m past its reach against a BF1 MinRange) and the sweep example's
  behaviour counters (Overwatch made it panic on every map; ci does not run it).
- 2026-10-16 00:15-01:10 (lane 2, build/resume): **the recycle refusal is NOISE on today's main.** Rebased snpr onto
  main eb331e7 (clean; now 761e37d). Recycles, all 32 maps, both binaries on one base (`lead_tmp/snpr/sw/M7_b1-6`,
  `B7_b1-6`, `rec48_allmaps.txt`): 48 seeds main 203 -> branch 201; per 8-seed block +22, +9, -7, -17, -3, -6 (main's
  own blocks: 19 / 28 / 35 / 42 / 34 / 45, so the gate's seeds 1-8 are the branch's worst block and main's best).
  Bespin 1 (seeds 1-24 14 -> 28) over 72 seeds: 76 -> 86, and the trap itself (within 10 m of (-6, -164), body 7
  `bes1_bldg_main02`, `bes1_trap72.txt`) holds bots LESS on the branch: 55,945 -> 51,211 unit-s, 60 s+ spells 260 -> 245;
  the same marksman (seed 9 unit 8) laps the same wall on both builds, identical at 60 s and still there at 187 s; the
  recycle fires or not by whether its lap reaches RECYCLE_ROAM (15 m; the lap is 14.3 m). Kashyyyk's pen (body 222)
  over 72 seeds (`kas2_pen72.txt`): recycles 44 -> 60 (p ~ 0.14), pen unit-s 8,313 -> 9,128 (paired t +1.25), 60 s+
  spells 13 -> 8, entries 1,765 -> 1,852; seed 1's rounds diverge long before the pen, so no same-case per-tick trace
  exists on both builds - the branch's case (`bb7/k/br`, a rifleman, not a sniper) slides on body 222 -> recover ->
  LOST -> recycled, main's own pattern (`bb/main/bf2_kas2_s2.json`). The gate's agg recycled (19 + 12) has no confirm
  block, so it will refuse again on seeds 1-8 (branch 41) by chaos: the planner either approves `recycled` for SNPR on
  this evidence or gives the agg a confirm run like the per-map values. Waits on the maintainer's STATUS 52 for the hold.
- 2026-10-16 11:03 (lane 1, land): **LANDED 6647001.** The first phase 1 (2174a7e) passed its gates but I aborted it:
  inside the approved stand20 were 10 spells of snipers standing 120 s in `travel` under a Snipe hint 2.5 m (Kamino
  8.7 m) over their feet, then recycled (main's rows: 1 spell). The mover's XZ-only arrival never fires off the post's
  region and upkeep's 2.0 m height test refused, so the walk neither ended nor failed (trace `lead_tmp/snpr/bbL/`).
  Fixed on the branch (6647001: given up under / over the spot on the last leg; `arrive` refuses it from below; test +
  2 mutations). Landed rows: travel-standing spells 10 -> 0, B9 standing with overwatch counted 0.05 % = main's, recycled
  35 -> 33. Report `reports/20261016_1103_land_SNPR.md`. Owed: the planner removes `[approvals.SNPR]`.
