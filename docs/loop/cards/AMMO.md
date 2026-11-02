---
id: AMMO
title: A primary weapon's reserve is the PS2's: WeaponAmmo N = one clip loaded + N-1 in reserve, and a value below 1 is an endless reserve (the port gives N spare clips and treats 0 as none)
milestone: M3
value: 3
phase: build
status: done
deps:
files: engine/crates/core/src/weapon.rs,engine/crates/core/src/unit.rs,engine/crates/game/src/roster.rs
declares: sim; sweep; arena
needs:
effort:
branch:
resume: MERGED into FPNT by the planner 2026-10-03 03:35 (not built; the batch rule): FPNT part 2 carries this card's text and acceptance
created: 2026-10-02
---
## Why
Card GREN (landed d4886ba, 2026-10-02) built the class slots with the PS2's ammo rule (analysis/re-grenades.md 2.5 [V, M]: B1 FUN_002838d8 - reserve clips = class ammo - 1, endless when the class value is below 1; RAM: a BF1 stormtrooper's rifle ammo 5 = reserve 4.0 + clip 1.0, its pistol endless) for every HELD slot (`WeaponState::with_clips`), and left the weapon in the hands on the old rule (`WeaponState::new`: N spare clips, 0 = no reserve) so that landing moved no sweep number. So the HUD shows 350 for the Rebel rifle (WeaponAmmo 6, 50 a clip) where the PS2 shows 300, and a primary authored with ammo 0 (BF1 clone pilot's arc caster, BF2 officer's chaingun, every sabre) has one clip and no reload.
