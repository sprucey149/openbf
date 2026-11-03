---
id: WTHL
title: Weather, lightning: the flashes on both Kaminos and BF1 Yavin 2 (dome and fog colour to white, sun and ambients rewritten - BF1 and BF2 in opposite senses), thunder per flash, and the sky bolt on the Kaminos (200 m ahead per group, 800 m out every 4-7 s) - the port has the resting dome tint only
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Kamino (both games) and BF1 Yavin 2 flash with lightning on the PS2: groups of 2-5 flashes every 3-5 s lift the sky
dome and the fog colour to white and rewrite the sun and the ambients for 0.2 s each; thunder plays per flash; on the
Kaminos a bolt stands in the sky for half a second, 200 m ahead of the player with every group and 800 m out every
4-7 s. The port has the resting dome tint only (card KSKY). The maintainer plays Kamino. Split from WTHR (the precipitation).

## Evidence
`analysis/re-weather.md` sections 7 and 8 ([O]; verifier 64 / 5 / 0 of 69 over the whole note):
- 7.1 the schedule (group count, the two gaps, when each sound plays); 7.2 the fade (b0 = B (1 - t/F), the flicker,
  dome f = SkyDomeDarkenFactor (1 - b), fog colour = the dome bytes, the restore);
- 7.3 the lights, DIFFERENT per game and proven in PS2 RAM: BF1 sun = Color (b - s) then the real sun climbing back;
  BF2 sun = Color (1 - b) (black at the peak) with s = 0; ambients k (saved + 0.333 saved sun) in both;
- 7.4 thunder: unpositioned, silent while every listener is inside a sound space;
- 8.1 the spawn (200 m / Y 200 -> 0 per group; 800 m / Y 300 -> 100 every 4-7 s; the look direction +- 0.5), 8.2 the
  class, 8.3 the shape (two LCG streams from the instance's seeds, n = len / BreakDistance, offsets perpendicular to
  the axis, the phase lerp, the taper, branches, the per-frame child) and the ribbon (half-width in metres, camera
  facing, additive, depth tested, no depth write, not fogged, alpha 0 at both ends).
The port already parses both blocks (`mungefs::fx::LightningConfig`, `LightningBoltConfig`) and carries the flash
fields in `ManifestLightning`; the bolt block is parsed and not baked (bake/src/map.rs 872).
Instruments: `lead_tmp/wthr/tools/lightning_ram.py <state>` (the Lightning object and the live lights against the
formulas), `bolt_census.py` (every live bolt in the kept states), `bolt_predict.py <bf1 state> [--export]` (rebuilds
a live bolt's polyline from its seeds and overlays it on the state's screenshot). Vector:
`lead_tmp/wthr/vectors/bf1_kam1_sideselect_bolt.json`.

## Acceptance (run and paste each)
1. Unit tests of the tick with the PS2's RAM rows: BF2 (`bf2_kam1_cw_sideselect`: t 0.0667, F 0.2, B 1, the b that
   gives dome byte 224 -> sun (0.3061 x3), ambient (0.4082, 0.4317, 0.4552), top (0.4293, 0.4528, 0.4763), bottom
   (0.3870, 0.4105, 0.4340) from saved sun (0.2549, 0.3333, 0.4118), ambient 0.3686, top 0.3922, bottom 0.3451) and
   BF1 (`bf1_kam1_cw_sideselect`: dome 193, sunlum 0.3333 -> sun (57, 57, 57), ambients (109, 115, 121) and
   (149, 154, 176) from saved sun (65, 85, 105), ambients (100, 100, 100) and (145, 143, 161)); the dome byte at
   rest 153 and at a peak 255; the schedule (a seeded run: N in 2..5 each about a quarter, group gaps in 3..5 s,
   one crack and N - 1 sub-cracks per group).
2. The bolt: the vector's strips (main 11 points, 2 branches) reproduced to 1e-3 from its seeds, phase and
   endpoints; the spawn rule against RAM (eye (-37.00, 93.46, 507.73) -> a top 200.0 m away horizontally at y 200,
   a foot 200 m lower within +-10 m); half-width 30 m tapering by 30 / 11 per inner point.
3. A headless or windowed run on BF2 KAM1 logs `kam_amb_thunder` once per group and `kam_amb_thundersub` for the
   further flashes; pictures for the maintainer of a flash peak and of a bolt beside a PS2 frame
   (`pcsx2/ref/bf1_kam1/pairs/cp0_cp2_beta1_v3_water.png`, `lead_tmp/wthr/out/bolt_sideselect_overlay.png`).
Mutation-check: BF2's sun with BF1's weight must fail test 1.

## Build notes (decisions taken in the RE; reversible)
- Viewer-side, its own LCG stream; the PS2 draws the child strip's seeds from the game's one generator at render
  time - the port must not touch the sim's.
- The still goldens and parity renders stay at rest (brightness 0, no bolt): a flash is time-driven; `declares:`
  expects no golden to move.
- BF2 really drives the directional light dark at a flash's peak (RAM); keep it - it is what the PS2 shows.
- Not established (re-weather.md section 11): the thunder sounds' own definitions (2D / 3D, gain), whether BF2's far
  bolts are on screen (BF1's are), the draw order inside the far pass, the HDR halving of the bolt's colour in BF2.
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.
- 2026-10-07 planner, WTHR's unchecked list (landed 1dbb18f; `reports/20261006_2350_card_WTHR.md`) - same files, take them here: (a) a splash ring sits over the sea edge in the one live screenshot (`lead_tmp/wthr/win/b1_2.png`); the splash ray takes terrain and ordnance bodies, not the water - what does the PS2 land a splash on? (b) the flake maps' parity rose (bf1_rhn1 +0.148, bf2_myg1 +0.094; limit 0.5) and the port's flakes look crisper than the PS2's in the zooms, cause not found; (c) BF1 and flake-map frame scores were not taken. `port_frame.py` / `pair.py` need a `pose_render` build (`--exe`).
