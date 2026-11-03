---
id: VREST
title: Parked flyers stand at the PS2's height: the pad's spawn point + the class's LandedHeight (authored, else minus the model's vis-box min y) - today every flyer is low by exactly that (gunship 4.15 m, X-wing 1.78, Y-wing 1.33, jedi fighter 1.10, snowspeeder 0.50); the treaded half went to HSTEP
status: archived
issue: 97
---

Historical technical record. Recorded state before archival was `done`. Current scope and ownership are on [issue #97](https://github.com/sprucey149/openbf/issues/97).

## Why (build, rewritten 2026-10-07 after the RE: `analysis/re-vehicle-rest.md`)
Every parked flyer stands too low in the port by exactly its class's LandedHeight: gunship 4.15 m (BF1 GEO1, BES1,
KAS1), X-wing 1.78 (BES1, KAS1, NAB1, TAT1), Y-wing 1.33, jedi fighter 1.10, snowspeeder 0.50 (both Hoths), geofighter
0.22 — 42 of the 70 pad flyers the port spawns. The PS2's flyer constructor adds LandedHeight (authored, else minus
the model's `modl INFO` vis-box min y) to the pad's spawn point, and nothing moves a landed flyer.
The treaded half of the title is NOT this card any more: hailfire and snail tank are hovers that rest on their
springs, and the port's fault there is the hover step's order — card HSTEP.

## Evidence
- `analysis/re-vehicle-rest.md` §1 (the pad's ray: pad + 1 m down to pad − 1 m, else the pad's own point), §2 (the
  rule and the table of every flyer class's LandedHeight on both discs), §4 (54 parked PS2 flyers).
- Today (main 11e7d61): `lead_tmp/vrest/port_census_main_10s.txt` joined with `ps2veh.jsonl` — the Flyer row reads
  21 of 54 within 2 cm (the TIEs and droid fighters, whose LandedHeight is under 1.2 cm).
- No PS2 frame shows a parked gunship. By the numbers its belly hangs about 1.5 m over the ground on the PS2 (origin
  4.15 m up, mesh floor −2.66 m): take a PCSX2 picture before anyone judges that by eye.

## Why (the RE card's, 2026-09-28)
VDRAW (2026-09-28) draws every pad vehicle at its sim transform. Hovers settle at SetAltitude (HOVR) and walkers' models start at y 0, but a flyer or treaded's origin is above its model's floor: rep_la_at_gunship ymin -2.66 m, all_fly_kwing -1.96, all_fly_xwing -1.78, all_fly_ywing -1.33, imp_fly_trooptrans -1.38, rep_fly_jedifighter -0.74, cis_tread_hailfire -2.06, cis_hover_mtt -0.82 (GLB bounds, side_bf1). Port picture: engine/artifacts/lead_tmp/vdraw/geo1_v2.png (the LAAT half under the sand).

## Evidence (the RE card's)
- core::sim::vehicles tick_vehicle_pads: height = ground_under (PROVISIONAL, VPAD).
- The PS2's landed flyer: re-vehicles §4.2 state 0 (landed), touchdown normal·up > 0.9375 — VFLY transcribes it; the rest height likely comes from the class's landing collision (p_ spheres / LandedHeight-style prop) — find it.
- Check: a PS2 RAM snapshot of a parked flyer (ps2live.py or a ref/states .p2s) — entity y minus terrain y at its xz.

## Note (PXWA, 2026-09-29, lane 2): BF1 rest heights from PS2 RAM (1-unit battle, pads parked)
`lead_tmp/pxwa/veh/<map>_table.md` (the PS2 entity matrix vs the port's VPAD spawn, all 15 BF1 maps, 213 vehicles; xz within
7 mm and heading within 0.06° for every parked class). The PS2 is HIGHER by: GEO1's LAAT gunships +4.15 / +4.40 m (the
port sinks them — this card's "half under the sand"), X-wing +1.776..1.783 (12), Y-wing +1.324..1.330 (3), HOT1's
snowspeeders +0.495 (4),
`com_weap_gunturret` +0.245..0.255 (112), fighter tanks +0.24..0.25, Geonosian fighters +0.21, speeder bikes
−0.03..+0.19; TIE fighter/bomber, AT-TE, AT-AT, tauntauns within 1.5 cm; AT-STs −0.40..+0.003, the hailfire −0.12. States with the
player at a post and every pad parked: `engine/artifacts/pcsx2/ref/states/<map>_walk_cp*_b000.p2s`.

- VHULL (2026-09-29): hulls now follow the sunk pose, so a sunk hull is a low wall. BF1 KAS2 pins a bot against a row of 3 AATs whose hulls sit 2.6 m under the ground (report reports/20260929_2109_card_VHULL.md). Resting the lowest body on the ground fixes both the drawing and the collision.

## Planner 2026-10-07 08:55 - value 4 -> 5
M2 has had no landing since 10-03 and four M2 research passes finished this morning. While lane 1 holds the bot files
lane 2's next picks at value 4 were six M1 research cards (dry run `lead_tmp/planner_1007b_dry.py`), and this build
came about the 30th pick. It is the most visible of the four: 42 of the 70 pad flyers stand 0.2 - 4.15 m too low
(the showcase Hoth's snowspeeders 0.5 m). The bot chain is not delayed in the dry run (lane 2 takes this while lane 1
is on OBSR / FGHTA / OBST2). Card VPAD2 also edits `game/src/vehicle_pads.rs`. Undo: `loopctl.py set VREST value=4`.

## Landed 2026-10-09 (82fb7db; report `reports/20261009_1051_card_VREST.md`)
- Acceptance 1: `every_flyer_class_reads_the_ps2s_landed_height` — 38 classes equal the table (14 BF1, 24 BF2),
  mutation-checked twice; `a_flyer_spawns_its_landed_height_over_the_pads_ray` (the ±1 m window), mutation-checked.
- Acceptance 2: `Flyer: 54 pads, mean |PS2-port| 0.0048, within 2 cm 53, within 5 cm 53, max 0.249` on main's
  rebaked bake (before: within 2 cm 21, max 4.399); 161 hover and 60 walker rows byte-identical to the before-run.
- Acceptance 3: `lead_tmp/vrest/build/pair_nab1_xwing.png`, `pair_geo1_gunship.png` (PS2 | after | before).
- Declares as landed: `rebake=side; sweep` (7 flyer maps moved; sim hashes, arena, leak, goldens, parity unmoved).
- Left: the pad ray is blind to vehicle hulls (PROVISIONAL); `laat3` +0.249 is FOLGC's; VFLY reads
  `PadClass::landed_height` for takeoff and landing.
