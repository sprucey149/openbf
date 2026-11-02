---
id: GRFL
title: Grenades fall through a deck a soldier stands on (the maintainer's playtest 2026-10-07, BF2 Kamino, on the platform where a turret is missing): the ordnance has its own collision layer and a soldier's floor can lack one - a grenade census over every floor a soldier can stand on, then fix each gap
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/sim/ordnance.rs,engine/crates/game/examples/ordnance_census.rs,engine/crates/game/examples/ordnance_census_accepted.txt
declares: sweep; arena
needs:
effort:
branch: grfl
resume: landed 296fa0c6 2026-10-09
created: 2026-10-07
alpha: yes
---

## Why
The maintainer, 2026-10-07: "when I would throw grenades on here, the grenades would fall through the floor and disappear. I couldn't fall through the floor though." Two Kamino decks; "it's platforms like these."

## Evidence (MEASURED 2026-10-09, main 5d83b7d; report `reports/20261009_1041_card_GRFL.md`)
- **The card's guess was wrong: the decks HAVE an ordnance floor.** the maintainer's deck (BF2 KAM1 103.1, 52.4, 75.0, `kam_bldg_aaa_bridge1`'s mesh, soldier + ordnance layers): a dropped grenade rests on it, 2 of 3 throws go through.
- **The cause is the sweep's ray, on every model floor of every map.** `Sim::sweep_sticky` (`core/src/sim/ordnance.rs`) starts its ray `SWEEP_BACK` 0.1 m BEHIND the previous position; a bounce puts the grenade `PUSH_OFF` 0.1 m off the face along the normal, and an impact steeper than about 32 deg loses all its tangential speed (re-grenades 4.3: f = 1), so it leaves exactly along the normal and the next tick's ray starts ON the face. The port's mesh ray (parry) meets a face from both sides and turns the normal toward the ray: the face is "hit" from behind, the normal points down, and the bounce puts the grenade 0.1 m UNDER the floor (trace: tick 15 bounce to y 52.473, tick 16 y 52.273 under the floor at 52.373). Terrain is not affected (its own segment test, card GRNP), which is why GRNP's Mos Eisley rows never showed it.
- **The census** (`lead_tmp/grfl/tools/grfl_census.rs`, 21 s for 32 maps: a 2 m grid of soldier floors over the nav graph, 4 real grenades per floor through `Sim::step` - a drop from 1.5 m and three 6 m/s level throws; THROUGH = the grenade's path crossed an up-facing SOLDIER-layer face going down): **127,017 of 1,556,912 grenades pass a floor a soldier stands on (8.2 %; 46,412 of 389,228 floors)**. Worst: BF2 Mygeeto 34,211 of 65,844 (52 %), Polis Massa 11,921, Utapau 10,297, Death Star 8,714, **Kamino 8,473 of 22,052 (38 %)**, Mustafar 7,315, BF1 Naboo Theed 7,232, Coruscant 6,437 of 7,608 (85 %). BF1 Bespin Platforms 0 of 25,852.
- **Prototype fixes on a scratch patch (`lead_tmp/grfl/tools/grfl_proto.patch`, switch `GRFL_FIX`), the same census:** 0 = main 127,017 · 1 = mesh front faces only 184,563 · 2 = mesh back faces only 90,947 (the baked collision meshes' windings are MIXED: neither one-sided ray works) · **3 = the bodies' ray starts AT the previous position, same end point: 7,260 (0.47 %)**, the maintainer's deck 0 of 4, core's 22 grenade tests green (the PS2 row-for-row throw is terrain: unchanged).
- **What is left under fix 3 is authored, not a sweep fault** (2,436 floors: 1,800 on a soldier-only body, 611 a throw off a deck's edge that then crosses one): Polis Massa 6,141 = the tops of `com_inv_col_*` invisible walls (`OrdnanceCollision = none`, unreachable "floors" the census takes on a map whose hub heights are not its floors); Kamino 937 = throws off `kam_bldg_mushplatform1` / `interior_right_entry` that cross the platform's soldier-only skirt prim at y 61.4; `com_weap_gunturret` tops, `fel1_bldg_bladder`, Ewok fences. The static check (no ordnance face within 0.4 m under a soldier floor) flags 6,500 floors, the same bodies plus deck edges a foot ring holds.
- NOT measured: what the PS2's mesh ray does behind its origin (one-sided, by its result: grenades rest on PS2 decks) - re-grenades 13.2 left "the bodies after the terrain" unread. No PCSX2 drop was made.

## Original card text (2026-10-07)

### Why
The maintainer, 2026-10-07: "when I would throw grenades on here, the grenades would fall through the floor and disappear. I couldn't fall through the floor though." Same platform as TSLG.

### Evidence
- A soldier is held up by the SOLDIER collision layer; a grenade by the ORDNANCE layer (`core::sim::ordnance` meets the map's ordnance-solid things: the ordnance layer, terrain, hulls; the aim ray does the same, `aim_hit`). A deck that has a soldier-layer floor and no ordnance-layer floor does exactly this. A GUESS until measured.
- GRNP (done) matched the grenade's bounce, roll and rest to the PS2 on Mos Eisley terrain; it did not look at decks.
- `leak_census` (card LEAK) does this for SOLDIERS (every wall, every deck); nothing does it for ordnance.

### Acceptance
- An ORDNANCE census: on every baked map, drop a grenade (and throw one at three headings) onto a grid of the floors a soldier can stand on (the soldier-layer floor under points the nav graph reaches), step the real sim, and flag every grenade that ends below the floor it started over (or under the map). Counts per map; the worst ten with positions.
- The maintainer's platform is in the list (KAM1, near his spot above or the TSLG platform), the cause is read (which body has no ordnance face, or a face on the wrong side), and each gap is fixed in the bake or the collision build so a grenade rests on every deck a soldier stands on. The PS2's own ordnance layer is the reference: a PS2 grenade dropped on the same deck (PCSX2) says whether the PS2 has the gap too.
- A gate: the census's `new` count is 0 (like `leak_census`), declared in `gates.toml`.

## Notes
Filed 2026-10-07 by Claude in chat from the maintainer's playtest (BF2 Kamino, F9 at tick 10701, build 6f6160b; snapshot `playtest/prev1.json`). His words are quoted; nothing here is measured unless it says so. Value 5: bots throw grenades too (GRAI), so a deck with no ordnance floor eats theirs as well.
- 2026-10-07 11:30 (the maintainer, second F9, via Claude): same finding on a second deck, KAM1 (103.1, 52.4, 75.0): grenades thrown on 'platforms like these' fall through the floor and disappear, the soldier does not (`playtest/playtest_20261007/5_kam1_platform_no_turret.png`, `playtest/playtest_20261007/f9_kam1_t3481.json`). Two decks (y 70 and y 52) on one map point at a whole kind of deck, not one body - start the census on KAM1's decks and platforms, then every map.
- 2026-10-09 12:34 LANDED 296fa0c (report `reports/20261009_1234_card_GRFL.md`): the bodies' ray starts at the previous position; test fails without it; census 127,017 -> 7,260 through (7,215 on soldier-only bodies, 45 on ordnance-layer bodies untraced), Kamino 8,473 -> 937, the maintainer's deck 2 of 4 -> 0 of 4. NOT done: the gate (oracle guard refused the gate-script and baseline edits: card GRFG), grenade kills before / after (no kill-by-weapon column; team kills 1,031 -> 1,120 in 8 seeds x 32 maps), the PS2 throw.
