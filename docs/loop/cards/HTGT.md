---
id: HTGT
title: The target's name and health on the HUD: what you shoot at (soldier, droid, vehicle, turret) shows its health at the top, the bar dropping as you hit it with bullets, grenades or rockets, coloured by team (friend blue, enemy red, neutral white) - the PS2 BF2 HUD's Target element, every map of both games
milestone: M3
value: 5
phase: build
status: done
deps: BHUD2
files: engine/crates/core/src/sim/input.rs, engine/crates/core/src/sim/state.rs, engine/crates/core/src/sim.rs, engine/crates/core/src/vehicle.rs, engine/crates/game/src/vehicle_pads.rs, engine/crates/game/src/event_feed.rs, engine/crates/game/src/blackbox.rs, engine/crates/game/examples/blackbox_read.rs, engine/crates/game/src/walk.rs, engine/crates/game/src/main/battle.rs, engine/CRATES.md
declares: sim
needs:
effort:
branch: htgt
resume: landed 90132e1a 2026-10-21 (slices 1-3; phase 2 bookkeeping by hand: land.py refused 'nothing to commit' on the previous landing's stale goldens/QUANT.md - card LANDG; no golden moved this landing)
created: 2026-10-20
first: 11
---

## Why
(the maintainer, 2026-10-20) The originals show the health of what you are shooting at at the top of the screen and it drops as you hit it - soldiers, and vehicles you hit with bullets, grenades or rockets. The maintainer does not remember whether the PS2 needs a hit first, a reticle pass or a hold of about a second, nor whether BF1 does it; he wants it on every map of both games. Colours as he remembers: teammate blue, enemy red, neutral (a droid) white.
BHUD3 lists 'the target's name and health' among a dozen HUD elements (value 4, not started); this card is that item alone, first.

## Evidence
`analysis/re-hud.md` (§2 element list, 'World markers: Target `player1target`', the reticle colour from `.weapon1.target.teamColor`); `Sim::reticle_pick` (the reticle's pick, already read by the HUD feed); card BHUD3 (the other HUD elements), BHUD2; `analysis/re-aim-assist.md` (the target the PS2 tracks).

## Notes
- 2026-10-21 AUDIT (PARTLY, card HTGTW): the IFT-X "bar 1.0 -> 0.67 under rifle fire" was taken with the face hook
  forcing the rifle's vehicle scale 0.1 -> 1.0 (`lead_tmp/htgt/tools/htgt_face_scratch3.diff`); the real BF2 rifle
  gives 1.0 -> 0.967 over the same ticks (`lead_tmp/audit-htgt/runB.log`). 10 of 14 audit mutations survive the suite
  (incl. the 3b install line and the readout's vehicle names) - HTGTW pins them.
- 2026-10-21 slice 3 LANDED 90132e1. Not covered: a building turret's bar never moves (shots do not damage its
  building: card VTHUL, evidence noted there); the BARC speeder's bar does not move under fire (its hull: card RKHUL);
  a destroyed turret is no candidate (the PS2 keeps showing a dead building at 0); HideHealthBar (two BF2 classes) not ported.
- 2026-10-21 RE done (report `reports/20261021_*_card_HTGT.md`). Build slices:
  1. **ui**: `hud_draw.rs` multiplies an element's drawn colour by its ancestors' (§6 of the RE: measured on the target
     bar AND the health bar); `hud_tree.rs` BarBitmap flash (`#2b76908f` / `#2deba703` / `flashyscale`, RE §5);
     `hud.rs` `RETICLE_COLORS` → BF2's (friend (1,86,213), team 0 / none white, neutral (223,223,32)) and a
     `HudModel.target` (name, fraction, relation, team-0 flag, shield) fed as RE §3 (disable + white on loss).
  2. **game**: `main/battle.rs` builds `HudModel.target` from `Sim::reticle_pick` (soldiers today): class display name
     via the baked Locl (`event_feed.rs` already reads `common_bf2/locl`), health / max.
  3. **core** (sim, declare it): widen the pick's candidates to vehicles and building turrets (`Surface::Vehicle`) as
     the PS2's acquisition does (RE §2: alive or a building, finite health, not the shooter's own vehicle) — the same
     winner feeds the magnet, so the player's assist can change (`re-aim-assist.md` §3.3's list-order rule).
  Props / buildings with health other than turrets are not in the sim (the PS2 picks them: Bookcase, Lava Bridge
  Control) — out of scope here.
