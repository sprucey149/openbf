# Version differences: measured census

Six supplied corpora (BF1 and BF2 on PS2, Xbox and PC) were compared against each other. PS2 is the gameplay
reference, so the question throughout is what a build without that game's PS2 disc would get wrong, and what could
stop it. `NM` means not measured, never zero or equal. PC BF1 is the supplied Steam Classic 2004 install; its
AddOn folder is excluded before selecting the base LVL tree.

The [measurement receipt](../scripts/data/version-diff/measurements.json) holds the counts and magnitudes these tables
are generated from, plus the hashes of the input LVLs and the tools. The
[input dependency audit](version-input-dependencies.md) covers shell, HUD, fonts and executable tables. The
[table design](gameplay-reference-table.md) describes the generated table and its limits.

## What a number table can fix and what it cannot

| Difference | Fixable by a number table? | State today |
|---|---|---|
| ODF class values (weapon, ordnance, health, speed) | **Yes.** They are numbers and names. | Reference values are in `ps2-v1`; correction integration is a separate bake step. |
| Direct mission declarations | **Yes**, for what the static reader can see. Branches, helpers and `ScriptPostLoad` stay `NM`. | Reference declarations in `ps2-v1`; complete effective missions remain NM. |
| Foliage placement lookup and grass atlas | **Yes.** | Reference numbers in `foliage-v1.json`; consumer equivalence needs a bake comparison. |
| Placements, regions, hints, barriers | **Yes.** Transforms and flags are numbers. | Measured below; correction coverage is separate from this raw census. |
| Paths and planning graphs | The existing fingerprinted correction instruction tables are approved (2026-11-02). | Compared whole below, including dynamic patches; correction coverage is separate. |
| Collision primitives (boxes, spheres, cylinders) | **Yes.** Each is a kind, three sizes and a transform. | Measured below; correction coverage is separate from this raw census. |
| One whole collision mesh shifted | **Yes**, a verified rigid offset. Renamed nodes are re-paired before this measurement. | Counts and offsets below; a shifted vertex set alone is not proof of matching polygon surfaces. |
| Terrain cutter planes | **Mostly.** A few numbers per cutter fix the simulated hole. The drawn clipped patch still comes from the donor. | Differs on BF1 Hoth and Rhen Var 1, BF2 Polis Massa; absolute correction records are required. |
| Font metrics, layout coordinates, clip lengths | **Yes.** | Not in the table. |
| **Terrain heights inside playable bounds** | **Yes.** The existing height rows and outside-bound height fills are approved for publication (2026-11-02). | Measured below; corrections must preserve the PS2 height query throughout playable cells. |
| **Collision meshes** | The existing collision correction programs are approved for publication (2026-11-02). | Measured below. |
| Animation tracks and root-motion curves | The existing animation and skeleton correction programs are approved for publication (2026-11-02). A duration or endpoint alone cannot reproduce a curve. | Measured below. |
| Whole assets one edition lacks (meshes, maps, clips) | **No.** | Counted below. |
| Art, audio, movies, fonts, localized text | **No**, and the repo must not hold them. | See the dependency audit. |

## Terrain: are the big numbers on playable ground?

**For PS2–Xbox, most changed samples are outside the authored bounds. Some smaller ones are in play, and some
large ones are hidden only by a bounding box.** The
33.30 m in BF1 (Endor) is beyond the map's boundary path and on patches the PS2 does not draw. The 16.08 m in both games
(Yavin 1) is beyond the boundary path. 96.7% of BF1's changed samples and 99.6% of BF2's are beyond the boundary. What
remains is real: 722 exposed samples in play across BF1's terrains, with soldier routes on Tatooine 1, Kashyyyk 2,
Naboo 1, Kashyyyk 1 and Hoth crossing changes of 1.2 to 2.9 m. Another 391 in-play samples sit under a placed object's
bounding box, up to 21.85 m on Hoth and 17.02 m on Rhen Var 2. Those were not checked against the object itself, so they
may be buried ground or open ground beside it. BF2 has one exposed sample, 0.60 m, and 65 under objects, up to 2.88 m,
all on Yavin 1, and every other BF2 terrain has identical heights.

PC BF1 adds a larger in-bounds discrepancy: Rhen Var 1 has exposed samples differing by **51.98 m**. Across its
12 distinct changed terrains, PC differs from PS2 at 35,444 samples: 33,948 outside the boundary, 876 exposed in play,
619 under objects, and one cut away at both heights. PC Rhen Var 2's overall 59.80 m peak is outside the boundary;
its largest exposed in-play sample is 2.30 m. These are decoded terrain differences, not collision-box artifacts.
The `BF1 PS2-PC` rows below keep them separate from the original Xbox comparison.

<!-- generated:terrain -->
| Game, map | Changed | Beyond boundary (largest) | Undrawn | Exposed in play: samples, >1 m, largest | In play under a placed object: samples, largest | Soldier arcs with exposed change >10 cm / >1 m (largest) / all | Spawn nodes, posts moved on terrain |
|---|---:|---:|---:|---:|---:|---:|---:|
| BF1 all, each terrain once | 33,624 | 32,511 | 1,780 | 722, 157 | 391 | | |
| BF1 end/end1.lvl | 3,769 | 3,575 (33.30 m) | 1,780 | 111, 42, 9.36 m | 83, 9.54 m | 1 / 0 (0.59 m) / 106 | 0, 0 |
| BF1 hot/hot1.lvl | 10,834 | 10,680 (23.00 m) | 0 | 121, 66, 2.99 m | 33, 21.85 m | 3 / 2 (1.20 m) / 57 | 0, 1 |
| BF1 kas/kas1.lvl | 75 | 0 (0.00 m) | 0 | 57, 15, 2.04 m | 18, 1.56 m | 23 / 5 (1.26 m) / 166 | 1, 0 |
| BF1 kas/kas2.lvl | 351 | 3 (0.36 m) | 0 | 287, 8, 2.16 m | 61, 5.76 m | 13 / 3 (2.02 m) / 190 | 1, 0 |
| BF1 nab/nab1.lvl | 72 | 0 (0.00 m) | 0 | 51, 5, 1.80 m | 21, 1.44 m | 4 / 2 (1.74 m) / 11 | 14, 0 |
| BF1 nab/nab2.lvl | 1,207 | 1,047 (5.22 m) | 0 | 73, 10, 5.16 m | 87, 5.16 m | 0 / 0 (0.00 m) / 135 | 0, 0 |
| BF1 rhn/rhn1.lvl | 383 | 381 (27.60 m) | 0 | 2, 2, 8.05 m | 0, 0.00 m | 0 / 0 (0.00 m) / 146 | 0, 0 |
| BF1 rhn/rhn2.lvl | 11 | 0 (0.00 m) | 0 | 1, 0, 0.46 m | 10, 17.02 m | 3 / 0 (0.74 m) / 116 | 0, 0 |
| BF1 tat/tat1.lvl | 31 | 0 (0.00 m) | 0 | 18, 9, 3.24 m | 13, 1.80 m | 8 / 1 (2.88 m) / 151 | 3, 0 |
| BF1 tat/tat2.lvl | 81 | 81 (15.06 m) | 0 | 0, 0, 0.00 m | 0, 0.00 m | 0 / 0 (0.00 m) / 230 | 0, 0 |
| BF1 yav/yav1.lvl | 16,810 | 16,744 (16.08 m) | 0 | 1, 0, 0.60 m | 65, 2.88 m | 1 / 0 (0.18 m) / 113 | 1, 0 |
| BF1 PS2-PC all, each terrain once | 35,444 | 33,948 | 1,912 | 876, 246 | 619 | | |
| BF1 PS2-PC bes/bes2.lvl | 134 | 16 (4.44 m) | 0 | 9, 0, 0.06 m | 109, 9.42 m | 1 / 1 (1.03 m) / 137 | 6, 0 |
| BF1 PS2-PC end/end1.lvl | 3,774 | 3,575 (33.30 m) | 1,780 | 111, 42, 9.36 m | 87, 9.54 m | 1 / 0 (0.36 m) / 106 | 0, 0 |
| BF1 PS2-PC hot/hot1.lvl | 10,912 | 10,680 (23.00 m) | 0 | 192, 111, 3.68 m | 40, 21.85 m | 11 / 8 (2.96 m) / 57 | 0, 1 |
| BF1 PS2-PC kas/kas1.lvl | 75 | 0 (0.00 m) | 0 | 57, 15, 2.04 m | 18, 1.56 m | 23 / 5 (1.26 m) / 166 | 1, 0 |
| BF1 PS2-PC kas/kas2.lvl | 353 | 3 (0.36 m) | 0 | 287, 8, 2.16 m | 63, 5.76 m | 13 / 3 (2.02 m) / 190 | 1, 0 |
| BF1 PS2-PC nab/nab1.lvl | 72 | 0 (0.00 m) | 0 | 51, 5, 1.80 m | 21, 1.44 m | 4 / 2 (1.74 m) / 11 | 14, 0 |
| BF1 PS2-PC nab/nab2.lvl | 1,207 | 1,047 (5.22 m) | 0 | 73, 10, 5.16 m | 87, 5.16 m | 0 / 0 (0.00 m) / 135 | 0, 0 |
| BF1 PS2-PC rhn/rhn1.lvl | 648 | 493 (27.60 m) | 0 | 67, 45, 51.98 m | 88, 16.10 m | 30 / 18 (8.23 m) / 146 | 9, 1 |
| BF1 PS2-PC rhn/rhn2.lvl | 1,347 | 1,309 (59.80 m) | 132 | 10, 1, 2.30 m | 28, 17.02 m | 14 / 3 (1.40 m) / 116 | 1, 0 |
| BF1 PS2-PC tat/tat1.lvl | 31 | 0 (0.00 m) | 0 | 18, 9, 3.24 m | 13, 1.80 m | 8 / 1 (2.88 m) / 151 | 3, 0 |
| BF1 PS2-PC tat/tat2.lvl | 81 | 81 (15.06 m) | 0 | 0, 0, 0.00 m | 0, 0.00 m | 0 / 0 (0.00 m) / 230 | 0, 0 |
| BF1 PS2-PC yav/yav1.lvl | 16,810 | 16,744 (16.08 m) | 0 | 1, 0, 0.60 m | 65, 2.88 m | 1 / 0 (0.18 m) / 113 | 1, 0 |
| BF2 all, each terrain once | 16,810 | 16,744 | 0 | 1, 0 | 65 | | |
| BF2 yav/yav1.lvl | 16,810 | 16,744 (16.08 m) | 0 | 1, 0, 0.60 m | 65, 2.88 m | 3 / 0 (0.18 m) / 340 | 15, 0 |
<!-- /generated -->

How to read it. Each column applies an exclusion to the vertices whose height differs between PS2 and Xbox:

- **Beyond boundary:** outside the map's authored `boundary` path. Of the 1,359 planning hubs on these maps, 1,351 lie
  inside that path, so it does outline the play area.
- **Undrawn:** every render patch touching the vertex has an empty layer mask on PS2.
- **Exposed in play:** inside the boundary, drawn, not inside a terrain cutter at both heights, and with no placed
  collision box over it. Bounding boxes over-cover, so this is a lower bound on the exposed share, and the excluded
  share may include open ground beside an object.
- **In play under a placed object:** the same, but with a placed collision box over the vertex. The two columns together
  are every changed sample inside the boundary that is drawn and not cut away in both versions.
- **Soldier arcs:** the simulation's own triangle height query under both grids, sampled every metre along each
  planning arc soldiers may use, keeping points with no placed object over them. Including points under objects the
  worst cases are 6.23 m (Rhen Var 2) and 5.74 m (Kashyyyk 2), both beside or under structures.
- **Spawn nodes and posts:** the query at each spawn path node and command post, counted when the point stands within
  2 m of the terrain. Kashyyyk 2's `cp4` moves by 0.36 m under a platform 23 m up, so it does not count; Hoth's `cp4`
  moves by 1.35 m and does.

The instrument was checked. `Cover.place` now uses the same column-major transform as `instance_matrix`; an
asymmetric rotated-box regression reproduced the old transpose. The correction moves the PS2–Xbox BF1 split from
721/392 to 722/391 exposed/covered samples without changing the 1,113 total. PC/Xbox PTCH masks are read from their
common 8-byte prefix; the optional 14-byte PS2 replacement header is checked separately. All 27,968 measured PS2
patch headers are 14 bytes (22,528 BF1 and 5,440 BF2), preserving their original decode path. Xbox BF1 has 8–11-byte
headers and the other donors 8–10. The old 14-byte requirement zeroed every donor painted mask: 22,528 BF1 Xbox,
21,504 BF1 PC, 5,440 BF2 Xbox and 11,328 BF2 PC patches.

The decoded PS2 heights and world coordinates equal the baked `terrain_*.glb` grid exactly
on all 12 original reference maps. The baked grid comes from the separate Rust reader. Spawn nodes sit a median 0.00 to 0.55 m from the
decoded ground, against 0.86 to 77 m with z flipped on maps with relief. All hubs on the grid land on drawn patches.

Limits. The port does not enforce the boundary path, so a soldier can walk past it unless the ground or a wall stops
him, and a flyer can always go there. Whether the PS2 stops them was not traced here. So "beyond the boundary" means
authored out of play, not unreachable. Which of those samples a player actually reaches on PS2 or in the port was not
measured, and the exposed count was not walked in a simulation. Changed cutters were not classified. PC BF2 has the same
heights as Xbox BF2 on all shared samples; its Dagobah grid is 256 per side against PS2's 128, adding 49,152 samples
beyond the PS2 grid, which the PS2's height query clamps at the edge.

Equal shared vertices alone do not establish equal terrain queries. Dagobah's 128×128 PS2 and 256×256 PC grids use
4 m cells but different float32 offsets. At the authored `cp5_spawn` point `(106.14756, 104.78419)`, their original
queries yield `1.5972964764` and `1.5973083973` m respectively; the difference starts a deterministic round diverging
on its first tick, with every unit still inside the PS2 extent. The correction table now retains the PS2 query size
even when it contains no height replacements. The bake writes optional `terrain_query_size`; the runtime takes the
centered reference window for simulation and preserves the complete donor render mesh. PS2 manifests omit the field
and remain byte-identical. A focused corrected PC Dagobah round matches every PS2 checkpoint and its final hash
`54fb70a5da6291a0` at tick 10,438. This verifies that case, not every source/map combination.

## Fog and sky ranges: authored values, before correction

The census now reads `sky_` config DATA slots directly from all six sources and keeps every authored argument.
The PC values are real source differences, not endian or float decoding errors. PC normally stores four
`NearSceneRange` arguments and two `FarSceneRange` arguments; PS2 normally stores three and one. The production
`SkyInfo` reader's `f3()`/`f32(0)` path truncates PC records: BF2 Dagobah's `20,30,60,80` becomes `20,30,60`, and BF1
Naboo Plains' `300,1000` far range becomes `300`. Treating either as PS2's schema is wrong. Canonical correction
records must replace the complete field with the PS2 target; a zero/default cannot stand for an absent authored field.

In the table, `N` and `F` are the complete near/far scene argument sequences; `fog` is FogRange, `RGB` is FogColor,
and `low` is LowResTerrain FogNear..FogFar. An omitted field is absent from that source config. "Identical to PS2"
compares all those fields, including the low-res fog colour and maximum distance retained in the receipt. Mode LVLs
are separate rows because the same map can carry different data. Shared `spa_sky` planet-dome records are counted
in the census comparisons but excluded from this map-range table. The BF2Vita overlay was used only as a lead:
78 of its SkyInfo fog/range rows were independently compared with this direct PS2-disc read, with zero differences.

<!-- generated:sky -->
| Game, map LVL | PS2 authored values | Xbox | PC |
|---|---|---|---|
| BF1 `bes/bes1.lvl` | N 0.5,200,500; F 2000; fog 0,2700; RGB 255,0,0; low 80..430 | identical to PS2 | N 90,240,120,280; F 300,1500; fog 0,2700; RGB 255,0,0; low 80..430 |
| BF1 `bes/bes2.lvl` | N 0.8,120,180; F 350; fog 250,350; RGB 128,128,128; low 80..430 | identical to PS2 | N 50,150,60,200; F 220,500; fog 250,350; RGB 128,128,128; low 80..430 |
| BF1 `end/end1.lvl` | N 0.7,100,150; F 0; low 90..350 | N 0.7,160,200; F 400; low 90..450 | N 50,180,60,220; F 160,400; low 90..450 |
| BF1 `geo/geo1.lvl` | N 2,210,280; F 3000; fog 0,3000; RGB 252,252,252; low 300..700 | identical to PS2 | N 50,220,60,300; F 450,1000; fog 0,3000; RGB 252,252,252; low 300..700 |
| BF1 `hot/hot1.lvl` | N 1,200,270; F 800; fog 0,3000; RGB 128,128,128; low 100..1000 | N 1,200,270; F 800; fog 0,3000; RGB 128,128,128; low 30..1000 | N 90,250,120,300; F 400,1400; fog 0,3000; RGB 128,128,128; low 30..1000 |
| BF1 `kam/kam1.lvl` | N 0.8,60,90; F 800; fog 600,1200; RGB 128,128,128; low 400..1000 | identical to PS2 | N 60,200,80,250; F 300,1000; fog 50,800; RGB 128,128,128; low 50..800 |
| BF1 `kas/kas1.lvl` | N 0.7,150,180; F 1000; low 500..1000 | N 0.7,150,180; F 1500; low 500..1000 | N 60,150,75,200; F 300,1500; fog 0,3000; RGB 128,128,128; low 9000..8000 |
| BF1 `kas/kas2.lvl` | N 2,80,100; F 1500; fog 0,3000; RGB 128,128,128; low 1000..2000 | N 2,120,160; F 1500; fog 0,3000; RGB 128,128,128; low 8000..9000 | N 60,150,75,200; F 180,1000; fog 0,3000; RGB 128,128,128; low 2000..4000 |
| BF1 `kas/kas2_h.lvl` | N 2,80,100; F 1500; fog 0,3000; RGB 128,128,128; low 1000..2000 | N 2,120,160; F 1500; fog 0,3000; RGB 128,128,128; low 8000..9000 | N 60,150,75,200; F 180,1000; fog 0,3000; RGB 128,128,128; low 2000..4000 |
| BF1 `nab/nab1.lvl` | N 0.95,100,130; F 1000; fog 0,3000; RGB 128,128,128; low 700..1000 | N 0.95,120,160; F 1000; fog 0,3000; RGB 128,128,128; low 700..1000 | N 70,150,90,200; F 300,1000; fog 0,3000; RGB 128,128,128; low 700..1000 |
| BF1 `nab/nab1_h.lvl` | N 0.95,100,130; F 1000; fog 0,3000; RGB 128,128,128; low 700..1000 | N 0.95,120,160; F 1000; fog 0,3000; RGB 128,128,128; low 700..1000 | N 70,150,90,200; F 300,1000; fog 0,3000; RGB 128,128,128; low 700..1000 |
| BF1 `nab/nab2.lvl` | N 1.75,100,150; F 3000; fog 0,30000; RGB 128,128,128; low 3000..6000 | identical to PS2 | N 60,180,80,220; F 220,800; fog 0,30000; RGB 128,128,128; low 3000..6000 |
| BF1 `nab/nab2_h.lvl` | N 1.75,100,150; F 3000; fog 0,30000; RGB 128,128,128; low 3000..6000 | identical to PS2 | N 60,180,80,220; F 220,800; fog 0,30000; RGB 128,128,128; low 3000..6000 |
| BF1 `rhn/rhn1.lvl` | N 0.7,100,150; F 100; fog 0,3000; RGB 200,200,200 | identical to PS2 | N 60,300,80,350; F 10,20; fog 0,3000; RGB 200,200,200 |
| BF1 `rhn/rhn2.lvl` | N 0.7,90,110; F 0; fog 0,3000; RGB 128,128,128 | identical to PS2 | N 60,170,70,220; F 30,40; fog 0,3000; RGB 128,128,128 |
| BF1 `tat/tat1.lvl` | N 1,100,150; F 2000; fog 0,2700; RGB 0,0,10; low 400..3500 | identical to PS2 | N 70,150,90,200; F 400,2000; fog 0,2700; RGB 0,0,10; low 400..3500 |
| BF1 `tat/tat1h.lvl` | N 1,100,150; F 2000; fog 0,2700; RGB 0,0,10; low 400..3500 | identical to PS2 | N 70,150,90,200; F 400,2000; fog 0,2700; RGB 0,0,10; low 400..3500 |
| BF1 `tat/tat2.lvl` | N 1.75,150,180; F 3000; fog 0,30000; RGB 128,128,128; low 200..3500 | identical to PS2 | N 50,150,60,200; F 150,1000; fog 0,30000; RGB 128,128,128; low 200..3500 |
| BF1 `test/test.lvl` | N 2,210,280; F 3000; fog 0,3000; RGB 252,252,252; low 300..700 | identical to PS2 | absent |
| BF1 `yav/yav1.lvl` | N 0.7,50,100; F 0; fog 1800,3000; RGB 128,128,128; low 90..350 | N 0.7,80,120; F 0; fog 1800,3000; RGB 128,128,128; low 90..350 | N 60,150,90,200; F 10,20; fog 1800,3000; RGB 128,128,128; low 90..350 |
| BF1 `yav/yav2.lvl` | N 0.7,100,150; F 350; fog 1800,3000; RGB 128,128,128; low 400..800 | N 0.7,110,140; F 350; fog 1800,3000; RGB 128,128,128; low 400..800 | N 60,150,80,200; F 140,400; fog 1800,3000; RGB 128,128,128; low 400..800 |
| BF1 `yav/yav2_h.lvl` | N 0.7,100,150; F 350; fog 1800,3000; RGB 128,128,128; low 400..800 | N 0.7,110,140; F 350; fog 1800,3000; RGB 128,128,128; low 400..800 | N 60,150,80,200; F 140,400; fog 1800,3000; RGB 128,128,128; low 400..800 |
| BF2 `cor/cor1.lvl` | N 0.1,150,200; F 1000 | N 0.5,180,250; F 0 | N 180,250,200,270; F 1000,1000 |
| BF2 `dag/dag1.lvl` | N 0.2,20,60; F 0; RGB 95,111,126 | N 0.5,20,80; F 0; RGB 95,111,126 | N 20,30,60,80; F 0; RGB 95,111,126 |
| BF2 `dea/dea1.lvl` | N 0.1,120,180; F 0 | identical to PS2 | N 120,120,180,180; F 0,0 |
| BF2 `end/end1.lvl` | N 0.7,90,150; F 0; fog 90,350; RGB 144,153,156 | N 0.7,100,130; F 400; fog 90,450; RGB 170,170,170 | N 40,120,50,150; F 160,400; fog 90,450; RGB 170,170,170 |
| BF2 `fel/fel1.lvl` | N 0.1,28,52; F 0; fog -10,62; RGB 80,100,80,128 | N 0.1,30,58; F 0; fog -10,62; RGB 80,100,80,128 | N 40,65,55,85; F 10,20; fog -10,62; RGB 80,100,80,128 |
| BF2 `gal/gal1.lvl` | N 0.1,2000,2000; F 3000 | N 0.1,3000,3000; F 3000 | N 0.1,2000,2000,2000,2000; F 3000,3000 |
| BF2 `geo/geo1.lvl` | N 0.2,180,200; F 500; fog 0,0; RGB 0,0,0 | N 0.5,210,680; F 700; fog 0,0; RGB 0,0,0 | N 50,220,60,300; F 450,1000; fog 0,0; RGB 0,0,0 |
| BF2 `hot/hot1.lvl` | N 0.5,200,270; F 800; fog 100,1000; RGB 203,201,203 | identical to PS2 | N 90,250,120,300; F 400,1400; fog 100,1000; RGB 203,201,203 |
| BF2 `kam/kam1.lvl` | N 0.8,60,90; F 800; fog 400,1356; RGB 0,0,0 | identical to PS2 | N 80,120,150,190; F 800,1000; fog 400,1356; RGB 0,0,0 |
| BF2 `kas/kas2.lvl` | N 0.5,85,100; F 5000; fog 0,0; RGB 104,106,108,128 | N 0.5,85,100; F 5000; fog -100,600; RGB 104,106,108,128 | N 30,160,40,200; F 5000; fog -100,600; RGB 104,106,108,128; low 0..0 |
| BF2 `mus/mus1.lvl` | N 0.5,80,110; F 2000; RGB 198,12,8 | N 0.5,200,250; F 2000; RGB 198,12,8 | N 250,350,300,400; F 1800,2000; RGB 198,12,8 |
| BF2 `myg/myg1.lvl` | N 0.5,50,80; F 300; fog 0.5,600; RGB 194,197,199 | N 0.5,60,100; F 1000; fog 0.5,600; RGB 194,197,199 | N 30,150,40,200; F 1000; fog 0.5,600; RGB 194,197,199 |
| BF2 `nab/nab2.lvl` | N 1.75,100,150; F 0 | N 0.5,100,150; F 0 | N 100,180,140,220; F 0 |
| BF2 `pol/pol1.lvl` | N 0.2,170,200; F 1000; fog 0,0; RGB 180,180,200; low 0..0 | N 0.5,170,200; F 2000; fog 0,0; RGB 180,180,200; low 0..0 | N 70,150,90,200; F 400,2000; fog 0,0; RGB 180,180,200; low 0..0 |
| BF2 `rhn/rhn2.lvl` | absent | absent | N 60,170,70,220; F 30,40 |
| BF2 `spa/spa1.lvl` | N 0.3,1000,1100; F 7000 | identical to PS2 | N 1000,1100,1500,1600; F 7000,8000 |
| BF2 `spa/spa2.lvl` | N 0.3,1000,1100; F 7000 | identical to PS2 | N 1000,1100,1000,1100; F 7000,8000 |
| BF2 `spa/spa3.lvl` | N 0.3,1000,1100; F 7000 | identical to PS2 | N 1000,1100,1500,1600; F 7000,8000 |
| BF2 `spa/spa4.lvl` | N 0.3,1000,1100; F 7000 | identical to PS2 | N 1000,1100,1500,1600; F 7000,8000 |
| BF2 `spa/spa5.lvl` | N 0.3,1000,1100; F 4000 | identical to PS2 | N 1000,1100,1500,1600; F 4000,5000 |
| BF2 `spa/spa6.lvl` | N 0.3,1000,1100; F 7000 | identical to PS2 | N 1000,1100,1500,1600; F 7000,8000 |
| BF2 `spa/spa7.lvl` | N 0.3,1000,1100; F 7000 | identical to PS2 | N 1000,1100,1500,1600; F 7000,8000 |
| BF2 `spa/spa8.lvl` | N 0.3,1000,1100; F 7000 | identical to PS2 | N 1000,1100,1500,1600; F 7000,8000 |
| BF2 `spa/spa9.lvl` | N 0.3,1000,1100; F 7000 | identical to PS2 | N 1000,1100,1500,1600; F 7000,8000 |
| BF2 `tan/tan1.lvl` | N 0.5,60,110; F 150 | identical to PS2 | N 60,80,80,110; F 400,2000 |
| BF2 `tat/tat2.lvl` | N 0.1,70,100; F 3000; low 0..0 | N 0.2,90,130; F 3000; low 0..0 | N 45,120,65,170; F 300,1000; low 0..0 |
| BF2 `tat/tat3.lvl` | N 0.2,20,60; F 0; fog 20,40; RGB 0,0,0 | N 0.2,30,70; F 0; fog 20,40; RGB 0,0,0 | N 20,70,30,80; F 0,0; fog 20,40; RGB 0,0,0 |
| BF2 `test/test1.lvl` | absent | absent | N 60,5000,90,6000; F 0,0; fog 0,6000; RGB 0,0,0,128 |
| BF2 `uta/uta1.lvl` | N 0.1,40,55; F 400 | N 0.1,55,75; F 600 | N 55,70,70,120; F 400,600 |
| BF2 `yav/yav1.lvl` | N 0.7,50,100; F 0; fog 90,350; RGB 144,153,156 | N 0.7,80,120; F 0; fog 90,350; RGB 144,153,156 | N 60,150,90,200; F 0,0; fog 90,350; RGB 144,153,156 |
<!-- /generated -->

## Collision meshes

A changed fingerprint does not always mean a changed shape, so changed meshes are split by what differs (the counts
tables below). Then, for meshes whose polygons differ, this measures the largest distance from a vertex of either
version to the nearest vertex of the other. That is an upper bound on a surface that moved, and an overestimate when a
flat wall was merely retessellated.

<!-- generated:collision -->
| Pair | Matched | Different surfaces | ≤1 cm | ≤10 cm | ≤1 m | ≤5 m | >5 m | Whole-mesh shifts | Largest |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| BF1 PS2 – BF1 XBOX | 743 | 67 | 8 | 2 | 16 | 28 | 13 | 0 | `nab2_bldg_b_04` 58.8 m |
| BF1 PS2 – BF1 PC | 705 | 100 | 15 | 5 | 22 | 42 | 16 | 2 | `bes2_bldg_chamber_exit` 127.7 m |
| BF1 XBOX – BF1 PC | 711 | 51 | 11 | 9 | 7 | 18 | 6 | 2 | `bes2_bldg_chamber_exit` 127.7 m |
| BF2 PS2 – BF2 XBOX | 1,417 | 93 | 20 | 3 | 21 | 27 | 22 | 0 | `all_cap_rebelcruiser2` 51.1 m |
| BF2 PS2 – BF2 PC | 1,414 | 153 | 30 | 4 | 46 | 36 | 37 | 0 | `kam_bldg_mushplatform1` 94.6 m |
| BF2 XBOX – BF2 PC | 1,421 | 79 | 17 | 3 | 26 | 16 | 17 | 0 | `kam_bldg_mushplatform1` 94.6 m |
<!-- /generated -->

Most of the largest gaps are capital ships in space maps and buildings on Tatooine, Rhen Var and Naboo 2. Which of them
a soldier can touch was not measured. Counts and fingerprints alone cannot supply changed polygon surfaces; the approved correction programs are a separate path. BF2's
`rep_hover_barcspeeder` is within 1 mm of a 1 m y translation on Xbox and PC, but adding that offset does not recover
the exact PS2 float32 polygon fingerprint. It is not counted as an exact whole-mesh shift. Candidate shifts are
counted only when the full translated polygon fingerprint matches; equal vertex sets alone are insufficient. Standalone meshes and other asset payloads remain refused. The existing collision correction instruction tables, including written coordinates and polygon instructions, were approved on 2026-11-02.

## Builds with no PS2 disc for that game

The 2026-11-01 policy requires the same simulation inside playable map bounds from every supported source.
openbf's gameplay rules are canonical, and outside-bound differences are acceptable for now, though a long ray from inside play can still read that ground (Yavin 1's hover gun does, from tick 293). Height rows reach one grid cell past the line, because ground inside it reads all four corners of its cell. Texture/asset resolution may differ.
PC BF1 need not keep a PS2 BF1 disc: public instruction tables over the player's own files are the complete correction path, map by map in `analysis/source-gates.md`. PC BF1 and PC BF2 together
build without a console disc; a single PC game alone is refused. Xbox discs can stand alone too. The census
below describes uncorrected source shapes, not acceptance of a source combination or proof of a corrected bake.

Same shapes per map, if a build took all ground and collision from the other edition:

<!-- generated:gate -->
| Pair | Maps | Same ground and collision surfaces, nothing missing | Of those, with other paths or planning | Side LVL meshes: other surfaces, missing | Other shared LVL meshes: other surfaces, missing |
|---|---:|---|---|---:|---:|
| BF1 PS2 – BF1 XBOX | 22 | 2: `kam1`, `test` | none | 9, 0 | 0, 0 |
| BF1 PS2 – BF1 PC | 22 | 1: `kam1` | none | 12, 2 | 1, 0 |
| BF2 PS2 – BF2 XBOX | 28 | 10: `cor1`, `dag1`, `dea1`, `gal1`, `geo1`, `kam1`, `mus1`, `myg1`, `nab2`, `tan1` | `cor1`, `kam1` | 14, 1 | 1, 0 |
| BF2 PS2 – BF2 PC | 28 | 4: `cor1`, `gal1`, `myg1`, `nab2` | `cor1` | 13, 3 | 4, 2 |
<!-- /generated -->

`test` is a developer map. The side column applies to every map using that side. The other shared LVLs are BF2's
`spa_sky`, the space maps' sky objects, and `ingame`, whose two meshes missing on PC are `com_icon_alliance_flag_carried`
and `com_item_healthrecharge_dst`. Primitive, placement, path and planning differences are not counted
against a map, because a table could carry those; a map with other paths does not match until one does.

The maintainer approved publication of the existing correction tables on 2026-11-02, including collision
coordinates, animation tracks, skeleton bones and height fills beyond the playable line. They remain instructions
over the player's own fingerprinted files. This does not authorize other game assets or remove the export's scrub
and data refusals. Parity checks must cover complete simulation queries, including outside samples read by a ray
originating inside the playable bounds, rather than merely counting changed in-polygon vertices.

A known-source fingerprint gate must reject an unknown revision before applying the records, and all correction
categories must apply together. A passing terrain/collision census is still weaker than a canonical hash of everything
the simulation reads plus the same scripted-round hash. Class, mission, object, fog, navigation, collision, water,
root motion and every other sim input need coverage; unfinished or unmeasured categories remain explicit holds.

## Sources and coverage

These are supplied-corpus comparisons, not claims about every patch, region or release. Console censuses read the
actual ISO9660 and XDVDFS discs. The PC BF2 install's patch and mod state is unverified; PC BF1 is the supplied Steam Classic 2004 base install. Addon directories are excluded,
and no DLC hero content was imported. BF1 PS2 `test/testh.lvl` is not UCFB and contributes nothing. There were no
geometry parse errors in the six corpora.

<!-- generated:corpus -->
| Corpus | UCFB LVLs | Class records | Mission records | Collision meshes | Skeletal clips | Candidate clip names |
|---|---:|---:|---:|---:|---:|---:|
| BF1 PS2 | 133 | 1558 | 44 | 761 | 413 | 291 |
| BF1 XBOX | 142 | 1588 | 44 | 810 | 681 | 488 |
| BF1 PC | 147 | 1519 | 44 | 724 | 422 | 291 |
| BF2 PS2 | 186 | 5362 | 132 | 1444 | 2016 | 585 |
| BF2 XBOX | 185 | 5350 | 132 | 1427 | 2012 | 582 |
| BF2 PC | 191 | 5463 | 138 | 1535 | 2016 | 584 |
<!-- /generated -->

## How to read the counts

A cell is **changed / matched**, excluding unmatched records and `NM` values. ODF properties match by file, sublevel
hash chain, class kind and name, duplicate occurrence, property hash and repeated-key occurrence. Values include numeric
scalars and vectors and identifiers, so rendering and sound properties count too. `Label` and other free text are `NM`.

Collision compares decoded positions and topology, then splits a change three ways: the same polygons in another order
(the same surface; whether a tie between contacts could resolve differently in the sim was not tested), the same
vertices joined differently, and different vertices. Primitives compare shape, size,
mask, parent and transform. Renamed collision nodes are re-paired only when exactly one unmatched mesh remains on each side with the same file,
sublevel and mesh name; ambiguous groups remain unmatched. Exact node identities take precedence. Named placements match by identity. Unnamed placements match exact class and transform, so
an insertion cannot falsely move every later object, and a moved unnamed object is unmatched. World records also include
regions, hints and barriers.

Mission rows are direct branch-free `ScriptInit` declaration leaves under initial literal state. No Lua VM executes.
Helper side effects, branches, callbacks and `ScriptPostLoad` stay `NM`, and complete effective mission values are `NM`
for every pair. Missing class minima are `NM`, not an invented 1.

Paths and planning graphs, including BF2 dynamic `plnp` patches, compare each whole resource by SHA-256, so a changed count does not say how far anything moved.

Animation identities are bank plus stored CRC, scope and duplicate occurrence. Candidate names are CRC matches to the
repository's source registry, so that column grows when source gains names and the census hash moves with it. The
receipt pins the registry it used. No other number depends on it. Root motion compares decoded `DummyRoot` translation curves; clips without that root are
`NM`. Generic world `anim`, all-bone poses and blend configuration are `NM`.

## BF1 measured counts

<!-- generated:bf1 -->
| Category | PS2–Xbox | PS2–PC | Xbox–PC |
|---|---:|---:|---:|
| Class records (all four kinds) | 128 / 1,542 | 409 / 1,503 | 328 / 1,510 |
| Entity ODF records (`entc`) | 67 / 893 | 135 / 855 | 91 / 861 |
| Weapon ODF records (`wpnc`) | 47 / 258 | 107 / 258 | 78 / 258 |
| Ordnance ODF records (`ordc`) | 13 / 204 | 124 / 204 | 117 / 204 |
| Explosion ODF records (`expc`) | 1 / 187 | 43 / 186 | 42 / 187 |
| Ordered property occurrence values | 190 / 37,290 | 777 / 37,077 | 619 / 37,384 |
| Direct mission declaration values | 8 / 2,448 | 58 / 2,434 | 50 / 2,434 |
| Effective mission execution | NM | NM | NM |
| Authored fog and sky ranges | 20 / 22 | 21 / 21 | 21 / 21 |
| Collision mesh records | 83 / 743 | 118 / 705 | 58 / 711 |
| Collision meshes, same polygons in another order | 16 / 743 | 18 / 705 | 7 / 711 |
| Collision meshes, same vertices, other polygons | 5 / 743 | 10 / 705 | 9 / 711 |
| Collision meshes, different vertices | 62 / 743 | 90 / 705 | 42 / 711 |
| Collision bounds | 45 / 743 | 69 / 705 | 33 / 711 |
| Collision primitives | 110 / 1,399 | 124 / 1,299 | 16 / 1,315 |
| Path and planning resources | 2 / 44 | 4 / 42 | 2 / 42 |
| Terrain records (heights/cutters/grid) | 15 / 22 | 16 / 21 | 7 / 21 |
| Terrain height fingerprints | 15 / 22 | 16 / 21 | 7 / 21 |
| Terrain cutter fingerprints | 2 / 22 | 2 / 21 | 1 / 21 |
| Terrain height samples at shared coordinates | 35,285 / 1,441,792 | 37,107 / 1,376,256 | 1,847 / 1,376,256 |
| Placed objects (`inst`) | 15 / 8,246 | 43 / 7,885 | 29 / 7,955 |
| All world records | 26 / 19,654 | 96 / 19,201 | 72 / 19,291 |
| Skeletal clip directory/root records | 14 / 413 | 20 / 412 | 6 / 414 |
| Clip frame counts | 1 / 413 | 2 / 412 | 1 / 414 |
| Gameplay root-motion trajectories | 2 / 384 | 3 / 383 | 1 / 385 |
| Corpus-only content names / union names | 703 / 7,089 | 1,034 / 7,243 | 469 / 6,937 |
<!-- /generated -->

## BF2 measured counts

<!-- generated:bf2 -->
| Category | PS2–Xbox | PS2–PC | Xbox–PC |
|---|---:|---:|---:|
| Class records (all four kinds) | 191 / 5,344 | 692 / 5,343 | 568 / 5,347 |
| Entity ODF records (`entc`) | 105 / 2,153 | 210 / 2,152 | 148 / 2,156 |
| Weapon ODF records (`wpnc`) | 50 / 1,320 | 301 / 1,320 | 251 / 1,320 |
| Ordnance ODF records (`ordc`) | 7 / 1,117 | 129 / 1,117 | 122 / 1,117 |
| Explosion ODF records (`expc`) | 29 / 754 | 52 / 754 | 47 / 754 |
| Ordered property occurrence values | 354 / 124,512 | 1,401 / 124,470 | 1,073 / 124,756 |
| Direct mission declaration values | 15 / 7,086 | 155 / 7,090 | 151 / 7,097 |
| Effective mission execution | NM | NM | NM |
| Authored fog and sky ranges | 15 / 44 | 28 / 44 | 28 / 44 |
| Collision mesh records | 103 / 1,417 | 170 / 1,414 | 91 / 1,421 |
| Collision meshes, same polygons in another order | 10 / 1,417 | 17 / 1,414 | 12 / 1,421 |
| Collision meshes, same vertices, other polygons | 12 / 1,417 | 26 / 1,414 | 16 / 1,421 |
| Collision meshes, different vertices | 81 / 1,417 | 127 / 1,414 | 63 / 1,421 |
| Collision bounds | 47 / 1,417 | 76 / 1,414 | 34 / 1,421 |
| Collision primitives | 340 / 7,448 | 398 / 7,392 | 170 / 8,034 |
| Path and planning resources | 10 / 420 | 23 / 420 | 18 / 421 |
| Terrain records (heights/cutters/grid) | 2 / 28 | 3 / 28 | 1 / 28 |
| Terrain height fingerprints | 1 / 28 | 2 / 28 | 1 / 28 |
| Terrain cutter fingerprints | 1 / 28 | 1 / 28 | 0 / 28 |
| Terrain height samples at shared coordinates | 16,810 / 348,160 | 16,810 / 348,160 | 0 / 348,160 |
| Placed objects (`inst`) | 102 / 9,649 | 160 / 9,655 | 59 / 9,647 |
| All world records | 145 / 23,801 | 828 / 23,811 | 683 / 23,896 |
| Skeletal clip directory/root records | 2 / 2,012 | 2 / 2,014 | 0 / 2,012 |
| Clip frame counts | 0 / 2,012 | 0 / 2,014 | 0 / 2,012 |
| Gameplay root-motion trajectories | 0 / 2,006 | 0 / 2,008 | 0 / 2,006 |
| Corpus-only content names / union names | 620 / 12,681 | 1,390 / 13,421 | 1,052 / 13,292 |
<!-- /generated -->

## Unmatched records and unmeasured fields

`L/R` are left-only and right-only counts. They never enter the matched denominator. A name in only one corpus is not
proof of playable platform-exclusive content.

<!-- generated:unmatched -->
| Population | BF1 PS2–Xbox | BF1 PS2–PC | BF1 Xbox–PC | BF2 PS2–Xbox | BF2 PS2–PC | BF2 Xbox–PC |
|---|---:|---:|---:|---:|---:|---:|
| Classes L/R | 16/46 | 55/16 | 78/9 | 18/6 | 19/120 | 3/116 |
| Properties L/R; NM | 112/253; 170 | 190/468; 165 | 80/216; 163 | 134/282; 204 | 189/747; 204 | 58/468; 203 |
| Mission records L/R | 0/0 | 0/0 | 0/0 | 0/0 | 0/6 | 0/6 |
| Mission declaration leaves L/R; NM subtrees | 0/0; 0 | 14/0; 0 | 14/0; 0 | 4/11; 388 | 2/16; 386 | 0/7; 388 |
| Collision meshes L/R | 18/67 | 56/19 | 99/13 | 27/10 | 30/121 | 6/114 |
| Primitives L/R | 55/42 | 155/54 | 126/38 | 517/663 | 573/878 | 77/236 |
| Terrains L/R | 0/0 | 1/0 | 1/0 | 0/0 | 0/2 | 0/2 |
| Placed objects L/R | 106/74 | 467/155 | 365/85 | 22/8 | 16/1023 | 10/1031 |
| Animation records L/R; root-motion NM | 0/268; 29 | 1/10; 29 | 267/8; 29 | 4/0; 6 | 2/2; 6 | 0/4; 6 |
| Scoped content occurrences L/R | 738/1406 | 863/915 | 1216/600 | 709/793 | 668/1894 | 265/1407 |
| Unique content names L/R | 375/328 | 552/482 | 246/223 | 270/350 | 300/1090 | 171/881 |
<!-- /generated -->

## Largest numeric declaration differences

Ranked by absolute scalar difference across ODF and direct mission declarations, grouping identical changes copied
across scopes. These are extremes across unlike units, **not an impact ranking**: the first four BF1 rows control
foliage visibility. A `#N` suffix is the zero-based repeated-property occurrence. An unknown property keeps its hash.

### BF1

<!-- generated:largest-bf1 -->
| Name | Property | Left → right | Absolute delta | Pairs |
|---|---|---:|---:|---|
| `rep_walk_atte` | MaxHealth | 15000 → 24000 | 9000 | P–C, X–C |
| `rep_fly_gunship` | MaxHealth | 7800 → 2340 | 5460 | P–C, X–C |
| `all_fly_ywing` | MaxHealth | 7200 → 2160 | 5040 | P–C, X–C |
| `cis_fly_maf` | MaxHealth | 7200 → 2160 | 5040 | P–C, X–C |
| `imp_fly_tiebomber` | MaxHealth | 7200 → 2160 | 5040 | P–C, X–C |
| `all_fly_xwing` | MaxHealth | 6600 → 1980 | 4620 | P–C, X–C |
| `rep_fly_jedifighter` | MaxHealth | 6000 → 1800 | 4200 | P–C, X–C |
| `cis_fly_droidfighter` | MaxHealth | 5400 → 1620 | 3780 | P–C, X–C |
| `bes_fly_cloudcar` | MaxHealth | 5000 → 1500 | 3500 | P–C, X–C |
| `imp_fly_tiefighter` | MaxHealth | 4800 → 1600 | 3200 | P–C, X–C |
<!-- /generated -->

### BF2

<!-- generated:largest-bf2 -->
| Name | Property | Left → right | Absolute delta | Pairs |
|---|---|---:|---:|---|
| `rep_assultship_destruct_towers` | MaxHealth | 70000 → 64000 | 6000 | P–C, P–X |
| `com_weap_award_rifle_ord` | Velocity | 160 → 300 | 140 | P–C, X–C |
| `com_weap_inf_rifle_ord` | Velocity | 160 → 300 | 140 | P–C, X–C |
| `com_weap_veh_laser_ord` | Velocity | 160 → 300 | 140 | P–C, X–C |
| `all_weap_moncal_roundturret_exp` | 0x125399a6 | 100 → 15 | 85 | P–C, X–C |
| `cis_weap_bldg_boxturret_exp` | 0x125399a6 | 100 → 15 | 85 | P–C, X–C |
| `imp_weap_bldg_boxturret_exp` | 0x125399a6 | 100 → 15 | 85 | P–C, X–C |
| `all_weap_moncal_roundturret_exp` | Damage | 200 → 150 | 50 | P–C, X–C |
| `cis_weap_bldg_boxturret_exp` | Damage | 200 → 150 | 50 | P–C, X–C |
| `imp_weap_bldg_boxturret_exp` | Damage | 200 → 150 | 50 | P–C, X–C |
<!-- /generated -->

## Reproduce

```sh
python3 scripts/version_census.py scan DISC_OR_INSTALL --out corpus.json     # once per corpus
python3 scripts/version_heights.py compare LEFT RIGHT --out heights_PAIR.json
python3 scripts/version_collision.py LEFT RIGHT --out shapes/PAIR.json
python3 scripts/version_terrain_play.py PS2 XBOX engine/assets_baked --game GAME --files MAP.lvl ... --out terrain_GAME.json
python3 scripts/version_report.py corpus.json ... --heights heights_*.json --shapes shapes/*.json --terrain terrain_*.json \
    --out scripts/data/version-diff/measurements.json
python3 scripts/version_report.py --update-doc analysis/version-diff.md --receipt scripts/data/version-diff/measurements.json
```

Only the numerical receipt is committed. The census holds per-record detail and stays local.
