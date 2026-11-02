---
id: CLOTH
title: BF2 cloth slice 1: Hoth snowtroopers' skirts, clone capes, hero capes and robes are CLTH meshes listed by their model's gmod (not by ClothODF) - parse CLTH and bake each wearer's cloth into its soldier GLB, pinned rigid to its bones, two-sided (the player's own snowtrooper on HOT1)
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/mungefs/src/cloth.rs (new), engine/crates/mungefs/src/lib.rs, engine/crates/mungefs/src/entity.rs, engine/crates/bake/src/soldier.rs, engine/CRATES.md
declares: rebake=side; goldens=units/bf2_imp_inf_snowtrooper.png,units/bf2_rep_inf_arctrooper.png,units/bf2_rep_inf_clonecommander.png,units/bf2_rep_inf_ep3heavytrooper.png,units/bf2_rep_inf_ep3sniper.png
needs:
effort:
branch: cloth
resume:
created: 2026-10-16
alpha: yes
---
## Why
Filed by FXC's census (2026-10-16, `analysis/re-chunk-census.md` 4). BF2 hangs **cloth** - a separate textured mesh on a
bone, with collision capsules on the legs - on the soldiers whose class names it with `ClothODF`. The port parses no
`CLTH`, so those soldiers are drawn without it. On the showcase battle BF2 HOT1 the Imperial rifleman (the most common
unit), rocketeer and sniper all wear the snowtrooper's white skirt to the knees - including the player when he picks them,
centre screen: PS2 `engine/artifacts/pcsx2/ref/bf2_hot1/cp1_cp6_postA_spawn.png` (skirt) against the port's
`engine/artifacts/lead_tmp/two-tier-soldier/units_after/bf2_imp_inf_snowtrooper.png` (bare brown hips). Also Vader's cape
(the Imperial hero), and off the showcase: the ep3 clone sniper / heavy / commander capes, every Jedi / Sith robe, the CTF
flags.

## Evidence
- Disc: 58 CLTH chunks in 10 BF2 files (`lead_tmp/fxc/census.tsv`); children INFO (texture) NAME PRNT (bone) XFRM DATA
  (texture name, vertex count, the mesh) COLL (shapes on named bones). BF1 has none.
- B2: the level loader FUN_00177058 -> FUN_001d9be0 -> FUN_001d9ca8 (COLL into 0x60-byte entries), registered by name in
  the table at 0x686ba8. ELF strings `EntityCloth`, `EntityClothClass`, `ClothData`.
- `ClothODF` = 0xf4d25eda on imp_inf_rifleman_snow / rocketeer_snow / sniper_snow -> imp_inf_snowtrooper_skirt (54 vertices).

## What to find (phase re, bounded)
- The DATA layout (vertices, the triangles, UVs, which vertices are pinned to the bone), the COLL shapes.
- The cloth step (EntityCloth's update: integration, constraints, gravity / wind, collision with the COLL shapes) and its
  draw (skinned to PRNT, both faces?) - B2 around FUN_001d9618 / FUN_001deda0 and EntityCloth's vtable.
- A first build slice can draw the mesh pinned rigid to its bone (the look on a standing soldier) and add the step next.

## RE done (2026-10-16, `analysis/re-cloth.md` [O]) - build slice 1 (this card)
The answers (doc sections): the chunk layout (1: pinned = vertices 0..F-1, no index list; the texture is DATA's string),
who wears it (2: the **gmod INFO list** x a `cloth` class of that name x the CLTH chunk - `ClothODF` is NOT read by the
game; the Wookiee's dreads, cloaked Anakin's skirt and Dooku's low cape have no class and are never drawn), the class keys
and defaults (3), the step (4: Verlet + three distance lists + sphere / cylinder / box pushes, 1 iteration, dt <= 0.039,
gravity -9.8, wind (1,0,0) x 10 x random), the draw (5: two-sided, front normal, opaque unless Transparent / HardEdge).
**Slice 1 (bake-only, no simulation):**
1. `mungefs/src/cloth.rs` (new): parse CLTH as section 1's table (byte stream: strings are NOT padded inside DATA / COLL;
   `engine/artifacts/lead_tmp/cloth/tools/clth.py` is the reference reader - all 58 chunks end exactly on their DATA / COLL
   ends). `mungefs/src/entity.rs`: `GeomModel` keeps the gmod INFO cloth names (u32, f32, u32 count <= 8, NUL strings).
2. `bake/src/soldier.rs`: for a BF2 soldier model, each gmod-listed cloth with a BASE `cloth` class of that name in the
   side LVL and a CLTH of that name becomes one more skinned segment of the soldier GLB: vertex v in bind model space =
   v . XFRM . Bind(PRNT) (row vectors, the `skel` bind pose - `place.py` puts the skirt's pinned ring at y 1.105-1.226 on
   the pelvis 1.159, hem 0.472); joints: a pinned vertex i < F on its weight bone, a free vertex on the bone of the pinned
   vertex nearest in the rest mesh (snowtrooper: all bone_pelvis), weight 1; UVs as read (/2048 is already undone: the
   disc floats are the UVs); the segment's texture = DATA's texture (the same sheet as the body for the skirt); opaque
   unless the class sets Transparent (Chewbacca's fur) / HardEdge. The renderer already draws every face two-sided
   (cull_mode None), as the PS2 does for the cloth.
3. CRATES.md lines for cloth.rs and the changed files.
Expected moves: rebake=side; goldens = the unit stills of models that wear a cloth (`units/bf2_imp_inf_snowtrooper.png`,
`units/bf2_rep_inf_arctrooper.png`, `units/bf2_rep_inf_clonecommander.png`, `units/bf2_rep_inf_ep3heavytrooper.png`,
`units/bf2_rep_inf_ep3sniper.png` - check each gmod lists one), plus any map golden that shows a snowtrooper (bf2_hot1?).
Measure the baked skirt texels' luma (sheet v 0.64-1.0, u 0-0.30) before fixing acceptance (2)'s 170 threshold.
The step itself is card CLOTH2.

## Built (2026-10-16, branch cloth 257ae94 + 47b9724)
- (1) `mungefs/tests/cloth_disc.rs`: 58 / 58 CLTH chunks parse to the last byte; per-chunk N / F / T for six of them
  against clth.py (skirt 54 / 18 / 80, 4 cylinders). Mutation: strings padded to 4 -> FAILED; restored -> ok.
- Wearers baked (bake soldier, all 4 BF2 sides): imp_inf_snowtrooper (skirt), rep_inf_arctrooper / clonecommander /
  ep3heavytrooper / ep3sniper (capes); every other baked soldier none (the Wookiee's dreads have no class). Bake diff =
  those 5 GLBs + the new `rep_inf_clonecommander_cape.ktx2`.
- (2) Hip box of the standing pose (unit sheet x 260-350, y 400-500): luma > 170 9.2 % -> 52.9 %, median 117 -> 174
  (`lead_tmp/cloth/units/bf2_imp_inf_snowtrooper_{before,after}.png`).
- (3) PS2 `pcsx2/ref/bf2_hot1/cp1_cp6_postA_spawn.png` (from behind) beside the port on HOT1 from behind
  `lead_tmp/cloth/units/port_back_main_branch.png` (main | branch; the gallery clip did not resolve, bind pose): skirt
  from the belt to below the knee, same off-white - viewed. (The oracle guard refused a script that only read the PS2
  file to compose one picture; the two are separate files.)
- A cloth segment carries `extras.cloth`: AIM_SIZE (the aim error's size term) is the model's own box; the test now
  measures without cloth (with it the arc trooper read 0.9631 against 0.9514).
- Visible limit, for CLOTH2: rigid on the pelvis, the skirt cuts through the thighs on crouch / jump poses (unit sheets).
