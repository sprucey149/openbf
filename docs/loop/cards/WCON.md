---
id: WCON
title: A walker's body prim on the terrain, as the PS2's: a tauntaun or kaadu parked on a slope creeps downhill at 0.98 sin(slope) m/s and loses cos^2(slope) of every step on a climb (both games) - the port has no terrain contact for a non-foot prim and runs the walker's contacts after its legs, not between the fall and the legs
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
On the PS2 a walker whose `TerrainCollision` lists a BODY primitive meets the terrain with it (`analysis/re-walker.md`
§16 [O], card WALKB1's RE): the contact no leg pair claims zeroes v.y and pushes the walker out **along the terrain's
normal** (§9 C5's else branch + C8), and the collide pass runs **between** the fall and the legs (task keys 0 / 10 /
15, §13.1). Two classes have such a prim, on both discs: `all_walk_tauntaun` and `gun_walk_kaadu` (`p_collision`).
What a player sees:
- **a parked kaadu or tauntaun on a slope slides slowly downhill** (the KAAD audit, 2026-09-30): |v| = 0.98·sin(slope)
  m/s along the facet's fall line; on flat ground it stands still;
- **a ridden one climbs slower**: a step d covers d·cos²φ − 29.4·dt²·sin φ·cos φ of ground — 5 % less at 12°, 11 % at
  18°; level and downhill lose nothing. The feet-only walkers (AT-ST and the rest) lose nothing at any slope.
The port builds neither: `core::vehicle::walker::contacts` holds only §9 C7 (buildings) and runs last, after the legs
(which would zero v.y before the fall). Its kaadus stand still on their pads and its tauntauns climb at full pace.
Showcase maps touched: BF1 NAB1 (kaadus), BF2 HOT1 (tauntauns).

## Evidence
- The rule in both ELFs: B1 FUN_00217d50 (B1:128966-128994, 129222-129223) → FUN_00168c90 → FUN_00168ff0 (position
  += normal × depth); B2 FUN_0025c2b0 (B2:177284-177313, 177601). The order: `re-vehicle-rest.md` (keys 0 / 10 / 15),
  re-walker §13.1.
- The creep, PS2: `lead_tmp/walkb1/creep_fit.py` — |v| / (0.98·sin φ) = 0.9955-0.9974 in 21 windows, slopes 2.9°-14.1°.
  The NAB1 pads (state `pcsx2/ref/states/bf1_nab1_units1_gcw_sideselect.p2s`, each kaadu put back on its spawn point,
  `lead_tmp/audit-kaad/slide_reset_B.csv` / `_C.csv`): the pad at (21.25, −75.11) moves (+0.0866, −0.0156, −0.0864) m/s
  for 30 s; perma_spawn12's (−0.1901, −0.0385, 0.000) m/s; perma_spawn13's (flat) 0.000. v.y reads −0.98 on 908 of 908
  ticks of a creeping kaadu and 0 on a parked AT-ST (`lead_tmp/walkb1/human_nab1_k.csv`).
- The slope loss, PS2 (`b1_slope.py`, `b2_slope.py`): BF1 kaadu 0.9882 / 0.9524 / 0.8930 of the root step at 4.8° /
  11.1° / 17.7° (the model 0.9885 / 0.9531 / 0.8931); BF1 tauntaun 0.9890 / 0.9379 at 4.5° / 12.7°; BF2 tauntaun 0.957
  at 10.4°; BF1 AT-ST 0.9995 / 0.9995 / 0.9979 at 4.0° / 11.2° / 21.9°.
- The prims: kaadu `p_collision` = a sphere of radius 0.5 at (0, 0.5, 0) on dummyroot; tauntaun radius 1.5 at
  (0, 1.515, 0). The baked kaadu `p_r_foot` has radius 0.0 (its disc prim kind 1 is not decoded; the PS2 holds 0.22,
  the left foot's) — fix it in the bake if the contact needs the foot's radius.

## Acceptance (run each on the branch AND on its parent; paste both)
1. **The parked NAB1 kaadus** (a scratch example or `walker_drive`'s trace: the three kaadus unridden from their
   spawn points, velocity by least squares over seconds 1-6; the PS2's is constant there, later the walker crosses to
   the next terrain facet): the pad at (21.25, −75.11) within 5 % of (+0.0866, −0.0156, −0.0864) m/s (the PS2 holds it
   for 30 s), perma_spawn12's within 5 % of (−0.1901, −0.0385, 0.000) m/s, perma_spawn13's under 0.001 m/s. The
   parent: all three stand still (they sink 0.03 / 0.10 m and stay).
2. **The slope table** from a port run (a kaadu on `bf1_nab1` and a tauntaun on `bf2_hot1` driven at full throttle
   over rolling ground for 60 s; ground covered over root motion applied, by the path's slope, as `b1_slope.py`
   bands them): climbs of 2-8° within 0.005 of the model, 8-16° within 0.01, level ≥ 0.999; an AT-ST on the same
   ground ≥ 0.998 in every band (unchanged). The parent: ≥ 0.998 in every band for all three.
3. **Parked walkers elsewhere do not move:** a 40 s census of every parked walker of the 32 maps — only tauntauns and
   kaadus on slopes move, none more than 0.3 m/s; every AT-ST, spider, AT-AT and AT-TE within 1 mm of the parent's
   rest. (`lead_tmp/vrest/examples/vrest_census.rs` already walks the pads.)
4. The walker unit tests that the pass order moves are re-derived by hand and listed in the report; a new unit test
   pins the creep on a tilted plane (0.98·sin φ within 1 %) and fails with the contact off (mutation check).
5. The sweep: every round finishes; the walker maps' rows are the declared moves.

## Notes
- Filed by WALKB1's research pass (2026-10-07, `reports/20261007_0849_card_WALKB1.md`). Value 3: visible on two
  showcase maps but small (a slow slide, a few % on climbs). `deps: WALKB1` — both edit `vehicle_class/walker.rs`,
  and the kaadu's speed in Acceptance 2 is WALKB1's.
- The order change (contacts between the fall and the legs) also moves §9 C7's building contacts one step earlier for
  every walker; card WCLMB's bunker step-up should be re-run after it. HSTEP is the hover's twin of the same order.
- BF2's unclaimed terrain contact also sets the supported bit (E+0x2190 |= 0x40); BF1 has none. One integrator: take
  BF2's.
- No sleep state was seen on the PS2: the creeping kaadus never stop in the 30 s samples.

## WALKB1 build note (2026-10-09, `reports/20261009_1216_card_WALKB1.md`)
- **A standing spider is lifted 8-11 m and slides with the sticks released** [measured, the port]: `walker_drive
  bf1_geo1 cis_walk_spider`, phase "aim right" (no throttle, the body turning in place): y - terrain +11.08 m, 2.94 m/s;
  BF2's own on bf2_geo1 +8.37 m, 1.92 m/s (`lead_tmp/walkb1/port_bf2_cis_walk_spider.txt`, before WALKB1). With the
  leg ray at BF1's 5 m instead of BF2's 10 it still moves 2.79 m/s there (159 ticks over 1.5 m off the terrain,
  against 247). Spot: about (122..136, -62, 1..9) on BF1 Geonosis. Cause not traced: C7's push-out or the leg ray
  on a prop are the candidates. Trace: `WALKER_TRACE=1`.

- 2026-10-09 12:45 planner (from WALKB1's report). The jungle AT-ST on BF1 Endor boosts to 5.83 m/s (settles at 5.0, slip p90 6.2 m/s) where the snow AT-ST with the same values reaches 7.23: not traced (`WALKER_TRACE=1` on `walker_drive`, end1). Also WALKB1's two own notes: a standing spider near a Geonosis rock is lifted 8-11 m (BF2's own spider did it before the card).

## Note from AUDIT HSTEP (2026-10-16)
HSTEP's reorder of the hover tick is held by ONE assertion (the skiff settle test's bob band): the parent's order and
both its rotations fail it, the mean and no-jitter asserts would not (`lead_tmp/audit-hstep/mut/`, M1 M2 M5). When this
card moves the walker's contact pass, pin the new order with a test that fails under every rotation of the old cycle
and assert the between-tick state the PS2 shows (a pending field cleared by the last task).
