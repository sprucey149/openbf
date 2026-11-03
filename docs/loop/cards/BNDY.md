---
id: BNDY
title: The map boundary: leaving the battlefield warns the player (HUD text + his team's 'leaving' line) and kills him after BF2's 6.5 s (BF1 10 s) - the port has no boundary at all; read the boundary data and the point test, then build it
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Filed by ANNC's RE (2026-09-30). On the PS2 a unit that leaves the map's boundary gets a HUD warning ("leaving the
battlefield"), hears his team's `<side>leaving` line and dies when a countdown runs out. The port has no boundary:
`core` has death regions only (`sim/state.rs` `death_regions`), so a player can walk off every map. This is the missing
half of ANNC's trigger T6.

## Evidence
`analysis/re-announcer.md` §2.4 ([V], verifier SUPPORTED on D1-D4):
- BF1 `FUN_00297e60(dt, member)` (B1 222656-222701): outside every boundary shape -> timer 10.0 s; the line once, for a
  local human; at 0 the unit is killed; back inside -> timer -1 (the next excursion starts again).
- BF2 `FUN_00350660` (B2 346687-346735): 6.5 s, or 2.1667 s when the unit's vehicle class has bit 1 of class+0x1208
  [H: a flyer]; the line is played by the HUD element `FUN_00391fe0` (389580-389610) when it first shows, with an
  alpha pulse; text key `game.tooltips.leaving_battlefield`. A soldier flag (bit 7 of soldier+0x660) forces the state
  [H: untraced].
- The boundary object: B1 `DAT_00518314`, B2 `DAT_005feca8`; a linked list of shapes built from named paths (B1
  `FUN_00295678` / `FUN_002957b8`, B2 `FUN_0034c4f0` / `FUN_0034c638`); "outside" = inside none of them
  (B1 `FUN_002956d8`, B2 `FUN_0034c550`); no shapes at all = never outside.
- Lines per mission: `lead_tmp/annc/bf1_calls.txt` / `bf2_calls.txt` (`SetOutOfBoundsVoiceOver`); five BF1 scripts set none.

## What to find
- Where the boundary comes from on the disc (the world's boundary / path chunk, the Lua call that names it), the shape
  test (segments of a path, which side is inside, the height rule if any), and which maps have one.
- Who is tested: every unit or humans only; AI behaviour at the boundary (do bots turn back?).
- The BF2 2.17 s case (which vehicles) and the soldier flag that forces the warning.

## Acceptance (rewritten 2026-10-19 after the RE - `analysis/re-boundary.md`, d63b63f)
RE half: DONE. The doc is [O] with a verifier workflow (19 claims, two readers each). Sheets for all 32 maps are in
`engine/artifacts/lead_tmp/bndy/sheets/`. The PS2's verdict agreed with the transcription on 296 of 296 points on the
six showcase maps plus BF1 KAM1 / HOT1, including 12 non-convex points the PS2 calls outside. BF1 YAV2, BF2 UTA1 and
BF2 MYG1 have NO boundary.

Build (the bars):
1. **Point replay**: the port's built test, run through the BAKED manifest, gives the PS2's verdict on every row of
   `engine/artifacts/lead_tmp/bndy/points/*.result.json` (404 rows: 296 + BF2 HOT1's second state). The rows are copied
   into a test fixture, and the KAM1 / HOT1 non-convex points are among them. Mutation: a point-in-polygon test in place
   of the half-planes must fail on those 12 points.
2. **Timer unit test**, all at 30 Hz like the PS2's M2:
   - outside at t0: the warning and the line once, at t0;
   - the timer is set and decremented in the same tick, and the unit dies in the tick it goes negative (t0 + 6.5 s);
   - back inside clears the timer, and the next excursion starts again from 6.5 s;
   - an AI unit dies the same way, and the cheat spares the player;
   - a map with no shapes never kills.
3. **PS2 pair on BF2 HOT1** (UTA1 has no boundary): the port's player is held at s0e0_out3 (295.43, 190.01). The warning
   starts in the first outside tick, and death comes within ±0.2 s of the PS2's 6.51 s after the first outside row
   (`points/bf2_hot1.bf2_hot1_walk_cp0_cp5_b000.result.json`, `death`).
4. **Sweep**: deaths by the new cause per map on 8 seeds, printed with their spots. BNDS keeps bots inside, so expect
   few; a rise is read spot by spot before landing.

Original acceptance (2026-09-30):
- RE doc with [V]/[O] claims and the verifier run; per map, the boundary polygon drawn over the port's top-down sheet.
- A PS2 check of the point test: for the six showcase maps, >= 20 sample points per map classified inside / outside by
  the PS2 (the live member timer after a RAM hold at each point, `walk_bf1.py` / `walk_bf2.py`'s hold) and by the
  transcription — 0 disagreements.
- Built: a unit test of the timer rule (outside at t0 -> warning + line at t0, death at t0 + 6.5 s, back inside
  resets), and a PS2 pair on BF2 UTA1: warning onset and death time within ±0.2 s of the PS2's.

## Notes
- PLAN plays BF2's rules on every map: default 6.5 s everywhere, BF1's 10 s recorded as data.
- ANNC's build leaves T6 to this card; the line names are already in ANNC's mission RON fields.

- Planner 2026-09-30 21:45: `alpha: yes` — every showcase map has a boundary, and PLAN's alpha checklist says "no soldier or AI out of bounds".

- Fable 2026-10-01: NOT this card's boundary (the 2-D leave-the-battlefield timer is still unbuilt), but related work landed: KFLR 5b12384 (a kill floor 15 m under the lowest playable floor; death regions and the floor kill past the invincibility cheat, as the PS2's do - `analysis/re-kill-floor.md` also confirms this card's boundary test is 2-D, x and z only) and BNDS 9440823 (bots stay inside the disc's BARR barriers at the level's edges and walk back when off it).
- Audit ANNC 2026-10-02 — the leaving lines carry locks the commander's other lines do not: BF2 `allleaving` has `PlayInterval 10` and every `repleaving` take is `(id, 1, 1, 0)` (a 1 s re-draw interval); `impleaving` / `cisleaving` and all four BF1 lines have none (`python3 engine/artifacts/lead_tmp/audit-annc/sndprop.py <SOUND LVL> leaving`). A request inside the interval plays nothing on the PS2, so the build needs the take / property lock for these two (memory `sound-take-locks-and-segs`).

## Note (card FXC, 2026-10-16): where the boundary comes from - the `bnd_` chunk
`analysis/re-chunk-census.md` 5 (verifier C8 recounted): every map LVL that has one carries a `bnd_` config chunk
`Boundary { Path(<hash>) ... }` (DATA 0x54eacca7 = FNV("boundary"), one 0x84874d36 = FNV("path") statement per path whose
argument is the path NAME's FNV); B2's level loader FUN_00177058 hands it to FUN_0034c3b0(DAT_005feca8, chunk) (B2 18380).
The paths are ordinary `path` chunks the port already reads (`mungefs::path`). Counts: BF1 one path on 18 maps
(`boundary` on BES1, NAB2(_H), RHN1/2, TAT1(H); `boundary1` on the other 11), **no `bnd_` on YAV1 / YAV2**; BF2 FEL1 4,
HOT1 3 (`boundary1..3`), SPA2 3, KAS2 / SPA7 / SPA9 2, eleven maps 1, and **an empty scope on COR1, DAG1, DEA1, MUS1, MYG1,
NAB2, TAN1, UTA1** - with no shapes the PS2 is never outside (2.4), so two BF2 showcase maps (UTA1, MYG1) and BF1 YAV2 have
no boundary unless the boundary object takes paths from elsewhere too: check FUN_0034c3b0 / B1 FUN_002954a8 for a second
source before building. Decoder: `engine/artifacts/lead_tmp/fxc/tools/bnd.py`.

## Note (lane 2, 2026-10-16 18:30, RE half done - DRAFT, not verified)
- Three of the six showcase maps have NO boundary on the PS2: BF1 YAV2 (no `bnd_`), BF2 UTA1 and MYG1 (empty scope) -
  the boundary object takes shapes only from `bnd_` (one caller of the push), and the list head is 0 in every
  YAV2 / UTA1 / MYG1 save state. The acceptance's "PS2 pair on BF2 UTA1" cannot be run: use BF2 HOT1 (3 shapes).
- The test is a CONVEX half-plane test per path (B1 FUN_002958f8 / B2 FUN_0034c780), union over paths, x and z only;
  BF1 BES1 / HOT1 / KAM1 paths are not convex, so the PS2 admits less than the drawn polygon there.
- The BF2 2.17 s is a flyer class with key 0xc6e9c80b = 1: only three guided-rocket ordnance classes on the disc,
  not starfighters. The "soldier flag" bit 7 of +0x660 is the FLYER's, with no writer found.
- RAM check done: all 32 baked maps' boundary edges in a save state = the transcription (271 edges, max 6.2e-5).
- Draft and tools: `engine/artifacts/lead_tmp/bndy/` (DRAFT_re-boundary.md, tools/, cmpshapes.txt, points/).

## Note (lane 2, 2026-10-19 16:00): RE DONE - `analysis/re-boundary.md` (d63b63f); the draft above is superseded
Three of the draft's readings turned out wrong:
1. Bit 7 of +0x660 has a writer: it is the BF2 flyer's FLY-HEIGHT flag, set when the flyer is below
   MinPlayerFlyHeight or above MaxPlayerFlyHeight. Such a flyer counts as outside even inside the 2-D boundary, so on
   UTA1 (29.5) and MYG1 (20) too. This is written as a note on card VFLY.
2. The kill's third argument 1 makes the invincibility cheat SPARE the player, unlike a death region (re-kill-floor 1f).
3. One BF2 AI flyer function (FUN_002faf90) reads the outside test, to refuse a placement point 300 m ahead. No
   foot-soldier code reads it.

Live results on the PS2:
- AI members are tested on BF1 (NAB1, TAT1, KAM1, HOT1) and BF2 (HOT1, full battle).
- Death comes at 10.0 s (BF1) and 6.51 s (BF2), with the timer set and decremented in the first outside update.
- The BF1 KAM1 / HOT1 / YAV2 walk states hold no player soldier; their `*_spawn` states were used instead.
- The point test agreed on 296 of 296 points (plus 108 of 108 on a second BF2 HOT1 state).

The build is in two commits (resume:); the files include `sim/step.rs`, which card TEAM holds, so BNDY becomes
eligible after TEAM lands.
