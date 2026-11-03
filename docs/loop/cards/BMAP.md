---
id: BMAP
title: The BF2 minimap and world markers: the Map element (the map's <world>_map masked by map_mask, FOV wedge, posts, targets, north pointer; small / large / spawn views) and the Target element (objective, flag, attacker, lock-on markers on / behind / off screen) — RE their placement and scale, then draw them in BHUD2's HUD
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why (RE done 2026-09-30, `analysis/re-hud-map.md`, commit 027dad9)
The PS2's minimap and world markers are now known and checked: heading-up, eye-centred, radius from the controlled
class's MapViewMin/Max × MapSpeedMin/Max (soldiers ±50 m), the `<world>_map` image over the `mapbounds` square (else the
posts' box + 100 m — BF1's rule), SetMapNorthAngle on the big maps, posts / units / Lua markers / FOV wedge / north
pointer, and the Target element's marker styles, off-screen clamp and distance scale (§2–§5). Predicted from RAM, 263 of
264 post icons land within ±3 px on 16 maps. The port has none of it (egui placeholder, no minimap).

## Original text (2026-09-30)
analysis/re-hud.md §3: the 1-player HUD's minimap (Map player1map at (0.856, 0.186), Scale 0.9) and its world markers
(Target player1target) are data in 1playerhud, but their DRAW is code the BHUD RE did not trace: how the <world>_map
image maps to world X/Z (bounds, zoom, whether it turns with the player), which icon and colour a command post gets per
team and while contested, how PositionLarge / PositionSpawn switch, and how Target markers scale with distance
(MinScale / MaxScale). The 1-player HUD shows CP capture only here (no commandPost bar outside 4playerhud).

## Notes

- CWSP RE (2026-10-02, `analysis/re-command-walker-post.md` section 5): a command walker's post has a map icon of its
  own — BF2 `FUN_003a6a48` draws each registered post at its OWNER's live position in the owner's team colour with
  the owner class's `MapTexture` / `MapScale` (AT-AT `atat_icon` 3.5, AT-TE `atte_icon` 2.2), rotated by the owner's
  heading (a fixed post's is not rotated), and prints the post's number (`HUDIndex`, "%d") only when `HUDIndexDisplay`
  is set — the AT-AT / AT-TE author 0, so no number. Activating the post removes the walker's ordinary vehicle marker.
  Frames: `lead_tmp/cwsp/shots/bf2_hot1_spawnmap.png` (two blue AT-AT icons, no number), `bf1_hot1_spawnmap.png`.

## Planner 2026-10-03 03:35 - value 4 -> 5; `files:` cleaned
The alpha's definition names "the real BF2 HUD"; the minimap is its largest missing element and is on screen in every
frame of a battle. The RE is done (027dad9) and BHUD2 has landed, so nothing holds the build.
`files:` had notes in brackets inside the list ("battle.rs (feed)"), which the picker compares as whole strings: it
would not have seen the overlap with SQHUD (`ui/src/hud.rs`, `game/src/main/battle.rs`). Plain paths now.
Undo: `loopctl.py set BMAP value=4`.
