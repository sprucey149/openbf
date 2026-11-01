# Version differences: measured census

Five supplied corpora (BF1 PS2 and Xbox; BF2 PS2, Xbox and PC) were compared against each other. PS2 is the gameplay
reference, so the question throughout is what a build without that game's PS2 disc would get wrong, and what could
stop it. `NM` means not measured, never zero or equal. There is no PC BF1 corpus here, so every PC BF1 cell stays `NM`.

The [measurement receipt](../scripts/data/version-diff/measurements.json) holds the counts and magnitudes these tables
are generated from, plus the hashes of the input LVLs and the tools. The
[input dependency audit](version-input-dependencies.md) covers shell, HUD, fonts and executable tables. The
[table design](gameplay-reference-table.md) describes the generated table and its limits.

## What a number table can fix and what it cannot

| Difference | Fixable by a number table? | State today |
|---|---|---|
| ODF class values (weapon, ordnance, health, speed) | **Yes.** They are numbers and names. | In `ps2-v1`. Not read by the game or the bake yet. |
| Direct mission declarations | **Yes**, for what the static reader can see. Branches, helpers and `ScriptPostLoad` stay `NM`. | In `ps2-v1`, unused. |
| Foliage placement lookup and grass atlas | **Yes.** | In `foliage-v1.json`, unused. |
| Placements, regions, hints, barriers | **Yes.** Transforms and flags are numbers. | Measured below. Not in the table. |
| Paths and planning graphs | **Yes.** Nodes, hubs and arcs are numbers. | Compared whole below, not by field. Not in the table. |
| Collision primitives (boxes, spheres, cylinders) | **Yes.** Each is a kind, three sizes and a transform. | Measured below. Not in the table. |
| One whole collision mesh shifted | **Yes**, an offset. Exactly one case: BF2 `rep_hover_barcspeeder`, 1 m in y. | Not in the table. |
| Terrain cutter planes | **Mostly.** A few numbers per cutter fix the simulated hole. The drawn clipped patch still comes from the donor. | Not in the table. Differs on BF1 Hoth and Rhen Var 1, BF2 Polis Massa. |
| Font metrics, layout coordinates, clip lengths | **Yes.** | Not in the table. |
| **Terrain heights** | **No.** A height grid is a shape. A short list of corrections is a judgement call (option 3 below), and a list long enough to make every sample match would be most of the changed region. | Measured below. |
| **Collision meshes** | **No**, apart from the one shift above. | Measured below. |
| Animation tracks and root-motion curves | **No.** A length or endpoint can be tabulated, the path between cannot. BF2 has 0 changed root-motion curves, BF1 has 2. | Measured below. |
| Whole assets one edition lacks (meshes, maps, clips) | **No.** | Counted below. |
| Art, audio, movies, fonts, localized text | **No**, and the repo must not hold them. | See the dependency audit. |

## Terrain: are the big numbers on playable ground?

**The largest numbers are not. Some smaller ones are, and some large ones are hidden only by a bounding box.** The
33.30 m in BF1 (Endor) is beyond the map's boundary path and on patches the PS2 does not draw. The 16.08 m in both games
(Yavin 1) is beyond the boundary path. 96.7% of BF1's changed samples and 99.6% of BF2's are beyond the boundary. What
remains is real: 721 exposed samples in play across BF1's terrains, with soldier routes on Tatooine 1, Kashyyyk 2,
Naboo 1, Kashyyyk 1 and Hoth crossing changes of 1.2 to 2.9 m. Another 392 in-play samples sit under a placed object's
bounding box, up to 21.85 m on Hoth and 17.02 m on Rhen Var 2. Those were not checked against the object itself, so they
may be buried ground or open ground beside it. BF2 has one exposed sample, 0.60 m, and 65 under objects, up to 2.88 m,
all on Yavin 1, and every other BF2 terrain has identical heights.

<!-- generated:terrain -->
| Game, map | Changed | Beyond boundary (largest) | Undrawn | Exposed in play: samples, >1 m, largest | In play under a placed object: samples, largest | Soldier arcs with exposed change >10 cm / >1 m (largest) / all | Spawn nodes, posts moved on terrain |
|---|---:|---:|---:|---:|---:|---:|---:|
| BF1 all, each terrain once | 33,624 | 32,511 | 1,780 | 721, 146 | 392 | | |
| BF1 end/end1.lvl | 3,769 | 3,575 (33.30 m) | 1,780 | 112, 39, 9.18 m | 82, 9.54 m | 2 / 0 (0.59 m) / 106 | 0, 0 |
| BF1 hot/hot1.lvl | 10,834 | 10,680 (23.00 m) | 0 | 121, 66, 2.99 m | 33, 21.85 m | 3 / 2 (1.20 m) / 57 | 0, 1 |
| BF1 kas/kas1.lvl | 75 | 0 (0.00 m) | 0 | 58, 15, 2.04 m | 17, 1.56 m | 28 / 5 (1.26 m) / 166 | 1, 0 |
| BF1 kas/kas2.lvl | 351 | 3 (0.36 m) | 0 | 293, 8, 2.16 m | 55, 5.76 m | 13 / 3 (2.02 m) / 190 | 1, 0 |
| BF1 nab/nab1.lvl | 72 | 0 (0.00 m) | 0 | 49, 4, 1.80 m | 23, 1.44 m | 4 / 2 (1.74 m) / 11 | 14, 0 |
| BF1 nab/nab2.lvl | 1,207 | 1,047 (5.22 m) | 0 | 64, 3, 5.16 m | 96, 5.16 m | 0 / 0 (0.00 m) / 135 | 0, 0 |
| BF1 rhn/rhn1.lvl | 383 | 381 (27.60 m) | 0 | 2, 2, 8.05 m | 0, 0.00 m | 0 / 0 (0.00 m) / 146 | 0, 0 |
| BF1 rhn/rhn2.lvl | 11 | 0 (0.00 m) | 0 | 3, 0, 0.46 m | 8, 17.02 m | 2 / 0 (0.43 m) / 116 | 0, 0 |
| BF1 tat/tat1.lvl | 31 | 0 (0.00 m) | 0 | 18, 9, 3.24 m | 13, 1.80 m | 8 / 1 (2.88 m) / 151 | 3, 0 |
| BF1 tat/tat2.lvl | 81 | 81 (15.06 m) | 0 | 0, 0, 0.00 m | 0, 0.00 m | 0 / 0 (0.00 m) / 230 | 0, 0 |
| BF1 yav/yav1.lvl | 16,810 | 16,744 (16.08 m) | 0 | 1, 0, 0.60 m | 65, 2.88 m | 1 / 0 (0.29 m) / 113 | 1, 0 |
| BF2 all, each terrain once | 16,810 | 16,744 | 0 | 1, 0 | 65 | | |
| BF2 yav/yav1.lvl | 16,810 | 16,744 (16.08 m) | 0 | 1, 0, 0.60 m | 65, 2.88 m | 3 / 0 (0.29 m) / 340 | 15, 0 |
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

The instrument was checked. The decoded PS2 heights and world coordinates equal the baked `terrain_*.glb` grid exactly
on all 12 maps. The baked grid comes from the separate Rust reader. Spawn nodes sit a median 0.00 to 0.55 m from the
decoded ground, against 0.86 to 77 m with z flipped on maps with relief. All hubs on the grid land on drawn patches.

Limits. The port does not enforce the boundary path, so a soldier can walk past it unless the ground or a wall stops
him, and a flyer can always go there. Whether the PS2 stops them was not traced here. So "beyond the boundary" means
authored out of play, not unreachable. Which of those samples a player actually reaches on PS2 or in the port was not
measured, and the exposed count was not walked in a simulation. Changed cutters were not classified. PC BF2 has the same
heights as Xbox BF2 on all shared samples; its Dagobah grid is 256 per side against PS2's 128, adding 49,152 samples
beyond the PS2 grid, which the PS2's height query clamps at the edge.

## Collision meshes

A changed fingerprint does not always mean a changed shape, so changed meshes are split by what differs (the counts
tables below). Then, for meshes whose polygons differ, this measures the largest distance from a vertex of either
version to the nearest vertex of the other. That is an upper bound on a surface that moved, and an overestimate when a
flat wall was merely retessellated.

<!-- generated:collision -->
| Pair | Matched | Different surfaces | ≤1 cm | ≤10 cm | ≤1 m | ≤5 m | >5 m | Whole-mesh shifts | Largest |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| BF1 PS2 – BF1 XBOX | 679 | 49 | 8 | 2 | 10 | 21 | 8 | 0 | `nab2_bldg_b_04` 58.8 m |
| BF2 PS2 – BF2 XBOX | 1,359 | 63 | 16 | 3 | 7 | 21 | 16 | 1 | `all_cap_rebelcruiser2` 51.1 m |
| BF2 PS2 – BF2 PC | 1,345 | 109 | 29 | 1 | 32 | 30 | 17 | 1 | `kam_bldg_mushplatform1` 94.6 m |
| BF2 XBOX – BF2 PC | 1,407 | 67 | 17 | 0 | 26 | 13 | 11 | 0 | `kam_bldg_mushplatform1` 94.6 m |
<!-- /generated -->

Most of the largest gaps are capital ships in space maps and buildings on Tatooine, Rhen Var and Naboo 2. Which of them
a soldier can touch was not measured. A table cannot supply any of these except the one shift. BF2's `rep_hover_barcspeeder`
is that shift: its collision sits 1 m higher in y on both Xbox and PC.

## Builds with no PS2 disc for that game

No such build exists today. `scripts/pack.py` refuses PC BF2 without a PS2 BF2 disc and PC BF1 without a PS2 BF1 disc,
and Xbox gameplay input is not wired in. The product rule still allows an Xbox BF2 disc to satisfy the PC requirement,
so this decides what that path may do. A number table alone does not make such a build play the same.

Same shapes per map, if a build took all ground and collision from the other edition:

<!-- generated:gate -->
| Pair | Maps | Same ground and collision surfaces, nothing missing | Of those, with other paths or planning | Side LVL meshes: other surfaces, missing | Other shared LVL meshes: other surfaces, missing |
|---|---:|---|---|---:|---:|
| BF1 PS2 – BF1 XBOX | 22 | 2: `kam1`, `test` | none | 4, 5 | 0, 0 |
| BF2 PS2 – BF2 XBOX | 28 | 9: `dag1`, `dea1`, `gal1`, `geo1`, `kam1`, `mus1`, `myg1`, `nab2`, `tan1` | `kam1` | 11, 4 | 1, 0 |
| BF2 PS2 – BF2 PC | 28 | 3: `gal1`, `myg1`, `nab2` | none | 10, 6 | 1, 6 |
<!-- /generated -->

`test` is a developer map. The side column applies to every map using that side. The other shared LVLs are BF2's
`spa_sky`, the space maps' sky objects, and `ingame`, whose two meshes missing on PC are `com_icon_alliance_flag_carried`
and `com_item_healthrecharge_dst`. Primitive, placement, path and planning differences are not counted
against a map, because a table could carry those; a map with other paths does not match until one does.

1. **Fingerprint gate, map by map (recommended first step).** Ship SHA-256 fingerprints of the PS2's decoded terrain,
   cutters, collision surfaces and root-motion curves: hashes only, no shapes. At bake, hash the donor's and allow only
   maps that match, reporting the rest as needing that game's PS2 disc. It fails closed. It covers 9 of BF2's 28 map LVLs
   on Xbox and 3 on PC, and only Kamino on BF1 Xbox, before shared LVLs. BF2's `gal1` is in both BF2 counts and is not
   one of the baked maps. Passing the gate means the same ground and collision meshes, not the same game: playing the
   same also needs placements, primitives and paths from a table (BF2 Kamino's paths differ on Xbox), and root motion,
   which this count does not include. Side vehicle and turret meshes need the same check, so those vehicles would need
   one of the options below. Whether a build that passes has the PS2's sim hash was not tested.
2. **Require the PS2 disc for that game.** The simplest and exact answer, and what happens today with PC input. It
   leaves out anyone who owns only the Xbox edition.
3. **Fingerprint gate plus a short list of height corrections.** Ship corrected heights for just the exposed in-play
   samples (721 in BF1, 1 in BF2) and apply them to the donor's grid. It removes the differences on open ground inside
   the boundary without shipping a grid, but it is shape data in a table, so it needs the maintainer's ruling against
   the content rule. It leaves samples beyond the boundary and under placed objects different, and a player can reach
   some of those, so sim hashes would not match a PS2 build if anything queries them. It fixes nothing about collision.
4. **Author replacement collision.** Write original shapes for the meshes that really moved, validate them against
   the decoded reference locally, and ship them as the project's own work. The only route that fixes collision without
   a PS2 disc. It is real modelling work (16 meshes over 5 m on BF2 Xbox, 8 on BF1 Xbox, before duplicates across LVLs),
   and hand-made shapes will not match exactly, so sim hashes would not either.
5. **Label it as a different game.** Build from the donor's own ground and collision, say so on screen, and refuse to
   mix with PS2-based builds in multiplayer or sim-hash comparison. Cheap, and honest about it, but it breaks the rule
   that every build plays the same, so it would need that rule changed.

## Sources and coverage

These are supplied-corpus comparisons, not claims about every patch, region or release. Console censuses read the
actual ISO9660 and XDVDFS discs. The PC BF2 install's patch and mod state is unverified. Addon directories are excluded,
and no DLC hero content was imported. BF1 PS2 `test/testh.lvl` is not UCFB and contributes nothing. There were no
geometry parse errors in the five corpora.

<!-- generated:corpus -->
| Corpus | UCFB LVLs | Class records | Mission records | Collision meshes | Skeletal clips | Candidate clip names |
|---|---:|---:|---:|---:|---:|---:|
| BF1 PS2 | 133 | 1558 | 44 | 761 | 413 | 291 |
| BF1 XBOX | 142 | 1588 | 44 | 810 | 681 | 488 |
| BF2 PS2 | 186 | 5362 | 132 | 1444 | 2016 | 585 |
| BF2 XBOX | 185 | 5350 | 132 | 1427 | 2012 | 582 |
| BF2 PC | 191 | 5463 | 138 | 1535 | 2016 | 584 |
| BF1 PC | NM | NM | NM | NM | NM | NM |
<!-- /generated -->

## How to read the counts

A cell is **changed / matched**, excluding unmatched records and `NM` values. ODF properties match by file, sublevel
hash chain, class kind and name, duplicate occurrence, property hash and repeated-key occurrence. Values include numeric
scalars and vectors and identifiers, so rendering and sound properties count too. `Label` and other free text are `NM`.

Collision compares decoded positions and topology, then splits a change three ways: the same polygons in another order
(the same surface; whether a tie between contacts could resolve differently in the sim was not tested), the same
vertices joined differently, and different vertices. Primitives compare shape, size,
mask, parent and transform. Named placements match by identity. Unnamed placements match exact class and transform, so
an insertion cannot falsely move every later object, and a moved unnamed object is unmatched. World records also include
regions, hints and barriers.

Mission rows are direct branch-free `ScriptInit` declaration leaves under initial literal state. No Lua VM executes.
Helper side effects, branches, callbacks and `ScriptPostLoad` stay `NM`, and complete effective mission values are `NM`
for every pair. Missing class minima are `NM`, not an invented 1.

Paths and planning graphs compare each whole resource by SHA-256, so a changed count does not say how far anything moved.

Animation identities are bank plus stored CRC, scope and duplicate occurrence. Candidate names are CRC matches to the
repository's source registry, so that column grows when source gains names and the census hash moves with it. The
receipt pins the registry it used. No other number depends on it. Root motion compares decoded `DummyRoot` translation curves; clips without that root are
`NM`. Generic world `anim`, all-bone poses and blend configuration are `NM`.

## BF1 measured counts

<!-- generated:bf1 -->
| Category | PS2–Xbox | PS2–PC | Xbox–PC |
|---|---:|---:|---:|
| Class records (all four kinds) | 128 / 1,542 | NM | NM |
| Entity ODF records (`entc`) | 67 / 893 | NM | NM |
| Weapon ODF records (`wpnc`) | 47 / 258 | NM | NM |
| Ordnance ODF records (`ordc`) | 13 / 204 | NM | NM |
| Explosion ODF records (`expc`) | 1 / 187 | NM | NM |
| Ordered property occurrence values | 190 / 37,290 | NM | NM |
| Direct mission declaration values | 8 / 2,448 | NM | NM |
| Effective mission execution | NM | NM | NM |
| Collision mesh records | 63 / 679 | NM | NM |
| Collision meshes, same polygons in another order | 14 / 679 | NM | NM |
| Collision meshes, same vertices, other polygons | 5 / 679 | NM | NM |
| Collision meshes, different vertices | 44 / 679 | NM | NM |
| Collision bounds | 34 / 679 | NM | NM |
| Collision primitives | 110 / 1,399 | NM | NM |
| Path and planning resources | 2 / 44 | NM | NM |
| Terrain records (heights/cutters/grid) | 15 / 22 | NM | NM |
| Terrain height fingerprints | 15 / 22 | NM | NM |
| Terrain cutter fingerprints | 2 / 22 | NM | NM |
| Terrain height samples at shared coordinates | 35,285 / 1,441,792 | NM | NM |
| Placed objects (`inst`) | 15 / 8,246 | NM | NM |
| All world records | 26 / 19,654 | NM | NM |
| Skeletal clip directory/root records | 14 / 413 | NM | NM |
| Clip frame counts | 1 / 413 | NM | NM |
| Gameplay root-motion trajectories | 2 / 384 | NM | NM |
| Corpus-only content names / union names | 703 / 7,089 | NM | NM |
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
| Collision mesh records | 72 / 1,359 | 125 / 1,345 | 79 / 1,407 |
| Collision meshes, same polygons in another order | 9 / 1,359 | 16 / 1,345 | 12 / 1,407 |
| Collision meshes, same vertices, other polygons | 12 / 1,359 | 26 / 1,345 | 16 / 1,407 |
| Collision meshes, different vertices | 51 / 1,359 | 83 / 1,345 | 51 / 1,407 |
| Collision bounds | 34 / 1,359 | 49 / 1,345 | 25 / 1,407 |
| Collision primitives | 340 / 7,448 | 398 / 7,392 | 170 / 8,034 |
| Path and planning resources | 10 / 394 | 22 / 394 | 17 / 395 |
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
| Population | BF1 PS2–Xbox | BF2 PS2–Xbox | BF2 PS2–PC | BF2 Xbox–PC |
|---|---:|---:|---:|---:|
| Classes L/R | 16/46 | 18/6 | 19/120 | 3/116 |
| Properties L/R; NM | 112/253; 170 | 134/282; 204 | 189/747; 204 | 58/468; 203 |
| Mission records L/R | 0/0 | 0/0 | 0/6 | 0/6 |
| Mission declaration leaves L/R; NM subtrees | 0/0; 0 | 4/11; 388 | 2/16; 386 | 0/7; 388 |
| Collision meshes L/R | 82/131 | 85/68 | 99/190 | 20/128 |
| Primitives L/R | 55/42 | 517/663 | 573/878 | 77/236 |
| Terrains L/R | 0/0 | 0/0 | 0/2 | 0/2 |
| Placed objects L/R | 106/74 | 22/8 | 16/1023 | 10/1031 |
| Animation records L/R; root-motion NM | 0/268; 29 | 4/0; 6 | 2/2; 6 | 0/4; 6 |
| Scoped content occurrences L/R | 738/1406 | 709/793 | 668/1894 | 265/1407 |
| Unique content names L/R | 375/328 | 270/350 | 300/1090 | 171/881 |
<!-- /generated -->

## Largest numeric declaration differences

Ranked by absolute scalar difference across ODF and direct mission declarations, grouping identical changes copied
across scopes. These are extremes across unlike units, **not an impact ranking**: the first four BF1 rows control
foliage visibility. A `#N` suffix is the zero-based repeated-property occurrence. An unknown property keeps its hash.

### BF1

<!-- generated:largest-bf1 -->
| Name | Property | Left → right | Absolute delta | Pairs |
|---|---|---:|---:|---|
| `nab2_prop_leafpatch` | MaxDistance | 80 → 1000 | 920 | P–X |
| `nab2_prop_leafpatch_tall` | MaxDistance | 80 → 1000 | 920 | P–X |
| `yav_prop_leafpatch` | MaxDistance | 120 → 1000 | 880 | P–X |
| `yav_prop_leafpatch2` | MaxDistance | 120 → 1000 | 880 | P–X |
| `kam_weap_bldg_AAA_turret_cannon` | LockOnRange | 200 → 0 | 200 | P–X |
| `Rhnlight` | OmniRadius | 50 → 15 | 35 | P–X |
| `nab_hover_gianspeeder` | DamageStartPercent #4 | 19 → 50 | 31 | P–X |
| `bes_weap_cloudcar_miss` | RoundsPerClip | 45 → 15 | 30 | P–X |
| `nab1_grass` | NumParticles | 60 → 30 | 30 | P–X |
| `cis_tread_hailfire` | DamageStartPercent #3 | 30 → 50 | 20 | P–X |
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

Only the receipt is committed. The census holds per-record detail and stays local.
