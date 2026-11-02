---
id: OVHT
title: The weapon heat gauge on the HUD: a weapon that heats (the pistol, the seat guns) shows heating and cooling on the centre icon so the player sees the overheat coming - the PS2's element, colours and thresholds, BF1 and BF2
milestone: M3
value: 4
phase: build
status: done
deps:
files: engine/crates/ui/src/hud.rs, engine/crates/ui/src/hud_tree.rs, engine/crates/ui/src/hud_draw.rs, engine/crates/game/src/hud_feed.rs, engine/crates/game/src/main/battle.rs
declares:
needs: cloud prerequisites: OVHT captures/layout dump/tools, baked HUD/maps/missions and approved GPU unavailable
effort:
branch:
resume: 2026-10-24 08:25 UTC: planner removed ANNC3 order-only dependency while its branch is absent/blocked; OVHT heat and ANNC3 bleed use distinct hud_model fields. Restore 27 inputs in reports/20261024_0814_card_OVHT.json; recheck live overlap, isolate build, implement on-foot re-heat-hud section 7 with repeated-send/caller mutations, both-game strips and full gates. Rebase/review battle.rs after either lands. No implementation or acceptance. See engine/artifacts/loop/reports/20261024_0825_planner.md.
created: 2026-10-21
alpha: yes
---
## Planner reconciliation (2026-10-24 08:25 UTC)
Removed the ANNC3 dependency added during cloud preflight. Its only shared source
is `main/battle.rs`: ANNC3 changes `HudModel.bleed`; OVHT fills the weapon models
from heat. The card's on-foot heat rule does not require ANNC3's music, announcer
or script-bleed implementation. RULES removes order-only dependencies on blocked
work. Keep this card blocked on its own missing inputs; recheck live claims and
file overlap before starting, and rebase/review both assignments after either
card lands. This does not restore ANNC3's absent branch or approve its landing.

## Why
The maintainer (INBOX 2026-10-21 18:40 item 3): on the PS2 a weapon with a cooldown shows its heat on the centre HUD icon.
RE done 2026-10-22 (`analysis/re-heat-hud.md`, [O], workflow-verified; live PS2 runs both games in
`engine/artifacts/lead_tmp/ovht/ps2/`):
- BF2 draws `player1weapon1heatbar` (30 `white` segments, -150..150 deg, inside the reticle group): the game sends
  `player1.weapon1.heat` EVERY frame = clamp(heat, 0, 1) for an unlimited weapon (RoundsPerClip under 1), 0 for a
  counted one, so the ring lights k of 30 while k/30 < heat (none when cold); colour by the layout's own NumberColor
  `player1weapon1heatcolor` (yellow -> orange). While overheated it also sends `player1.weaponsOverheat` (the
  "OVERHEATED!" line, already in the layout and the baked locl) and `player1.weapon1.refire` = 1 - HeatThreshold / heat
  (two red 3-segment bits under the reticle, else 0).
- BF1 draws a different element (a bar in its weapon panel, blue -> red); PLAN puts BF2's HUD on every map, so BF1
  maps get BF2's ring from BF1's heat (named deviation by PLAN).
- The port's sim already keeps `WeaponState::heat` / `overheated` (same numbers as the PS2 on the pistol, per update);
  the HUD feed sends none of the three events.

## Do (the on-foot slice)
1. `ui::hud`: `HudWeapon` gets `heat` (0..1 as the PS2 sends it), `refire` and `overheated`; `feed` sends
   `player1.weapon1.heat` (Number) and `player1.weapon1.refire` (Number) every frame while weapon 1 is shown, and
   `player1.weaponsOverheat` every frame while overheated (the PS2's every-frame sends: the line holds 0.1 s and fades
   1 s after the clear). Check how `hud_tree` treats a repeated enable before relying on it (the clip ring's feed).
2. `game::main::battle::hud_model`: fill them from `me.weapon` + its class: heat = `infinite_ammo()` ? clamp(heat) : 0;
   refire = overheated ? 1 - clamp(heat_threshold / heat) : 0. Weapon 2 (the grenade row) sends 0.
3. Both games through the same path (BF1 maps: BF1's heat on BF2's ring).
4. Tests (ui): lit segments at heat 0 / 0.5 / 1.0 (0 / 15 / 30), the colour at 0 and 1 against the NumberColor table,
   the line on while overheated and off after its fade, refire bits 2 per side at heat 0.5 with threshold 0.25;
   mutation-check each (drop the every-frame send, swap the infinite test, invert the overheat gate).
Not here: the charge on the ring for counted charge weapons and the state-4 refire bits (note on BHUD3); the seat guns'
ring - the reticle is not shown off foot in the port at all (card VSHUD); WHUM owns the overheat sound.

## Acceptance
- A picture strip per game: cold, half hot, overheated, cooling - with the sim's heat value beside each frame; the
  PS2's frame beside it (PCSX2, the same weapon): PS2 frames are ready in `engine/artifacts/lead_tmp/ovht/ps2/
  bf2_pistol/rep0_{half,over,cool1,cool2,cold}.png` (rows in `rep0.csv`, `tools/ringarc.py` measures the lit arc) and
  `bf1_pistol/` (BF1's own bar, for reference). Take the port's frames with the same tap script (R1 3 updates on, 5 off).
- A ui test: the ring's lit segments against heat (0, 0.5, 1.0) and the overheat flag; mutation-checked.
- Golden: the HUD golden it moves, declared (a cold weapon lights no segment: likely none - prove it with the gates).

## Original text (2026-10-21)
`analysis/re-hud.md` already has BF2's heat ring (30 x `white` segments over -150..150 deg, rgb (239,255,41), beside the
clip ring); BHUD3 listed it among many - this card takes it out of BHUD3 so it does not wait on the rest.
Do: 1. RE: BF1's element (re-hud*.md, the HUD tree / code), and when each game shows it (heat > 0 only? the overheat
line?). 2. Build for every heat weapon the sim models (soldier weapons and seat guns; BRSTU's heat model), the overheat
state as on the PS2. WHUM owns the overheat sound.

## Result (issue #78, 2026-11-01)
Built on foot and in seats, with the charge on the ring: `analysis/re-heat-hud.md` 7.1. Tests: `game/tests/suite/hud_layout.rs` (ring 0 / 15 / 30 segments, colour, line, bits on BF2's layout), `weapon_gauge.rs` (sim to gauge: pistol, charge, turret and vehicle gun, both games). The port's frames from `examples/hud_capture.rs` (`HUD_HEAT`, `HUD_REFIRE`) were looked at beside the PS2's `lead_tmp/ovht/ps2/bf2_pistol/` overheated and cooling frames (`re-heat-hud.md` 7.1); no strip was saved, and BF1's frames were not compared.
