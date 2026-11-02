---
id: HUDW
title: The HUD's weapon panel is drawn wrong (the maintainer's playtest 2026-10-04): BF1 Empire (KAS2) has a light-blue blob over the health bar under the rifle icon; BF2 Kamino's pistol shows the infinity ring over the clip count ('oo46'), a grenade icon overlapping the count pill and the gun icon stretched across the panel - pixel-measure both against PS2 frames of the same weapons, then fix
milestone: M1
value: 5
phase: build
status: done
deps:
files: engine/crates/ui/src/hud.rs, engine/crates/ui/src/hud_tree.rs, engine/crates/ui/src/hud_draw.rs, engine/crates/game/src/hud_feed.rs, scripts/rebake.sh
declares: rebake=side
needs:
effort:
branch: hudw
resume: landed 692fa469 2026-10-05
created: 2026-10-04
alpha: yes
---

## Why
The maintainer, 2026-10-04: "empire hud weapons has a weird blotch on it" and "pistol weapon hud is all messed up". The weapon panel is on screen all the time.

## Evidence
- `playtest/playtest_20261004/1_kas2_empire_hud_blotch.png` (BF1 KAS2, Empire, rifle): a light-blue ellipse over the health bar's middle, under the rifle icon; the top pill shows `4` with no icon.
- `playtest/playtest_20261004/3_kam1_pistol_hud.png` (BF2 KAM1, Republic, pistol): a ring with the infinity sign drawn over the clip count (reads `oo46`), the grenade icon overlapping the top pill, the gun icon stretched across the panel and over the pills.
- Not measured: which draw is the blob (draw-isolate it), and what the PS2 shows for the same weapons (PCSX2 frames).

## Findings and what was built (2026-10-05, lane 1; report reports/20261005_1215_card_HUDW.md)
Each defect draw-isolated (renders `lead_tmp/hudw/pics/`, PS2 frames `lead_tmp/hudw/ps2/`, 640x480):
1. **The blue blob (BF1 KAS2)** = row 2's icon sheet: the BF1 thermal detonator's `hud_com_grenade_icon` (32x32),
   drawn by `hud_draw::icon` at ROW 1's rifle spot under ROW 2's group -> (117, 406), on the health bar (the maintainer's blob
   centre measured (115, 406)). `HudView::update` ran the icon rule for row 1 only. Now per row; a BF1 class keys BF2's
   NameMesh table by its own name (BF1's `imp_weap_inf_thermaldetonator` -> `hud_thermaldetonator`, box equal to
   BF2's to 0.5 px); a grenade BF2 has no mesh for (crack grenade) shows its icon in the grenade's place.
2. **'oo46'** = the last weapon's count left under the infinity ring. PS2 [V]: an infinite count is sent as -1 and the
   Text prints "" (flag 0x7a4e6e71 = 0) or "--" (`FUN_003ae6c0` B2:406176-406193; ELF 0x630580 = "", 0x630588 = "--").
   PS2 frames (clone pistol, SBD tri-shot): the ring alone, no number; the count returns with the rifle.
3. **The grenade over the top pill is the PS2's layout** (PS2 frame s2_rifle.png and re/bhud/crop_weap_hot1.png) - not a defect.
4. **The 'stretched' gun** = the clone RIFLE's mesh left up under the pistol (no pistol model was baked, and no event
   cleared the mesh), drawn ghostly (its sheet's alpha plane is gloss). PS2: rifle x 93-224 rows 377-397; pistol
   x 78-175 rows 374-401, both solid. Port after: rifle 93.1-222.2 x 375.4-395.8, pistol 78.1-174.1 x 373.0-399.5,
   ring 35.6-64.1 x 372.5-397.8 (PS2 36-65 x 373-399). Pairs: `pics/pair_kam1_bright.png`.
Acceptance run: `cargo test -p game --test hud_layout` 9 passed on the shared bake and on a scratch bake with the
pistols (`OPENBF_ASSETS`); two mutations (no -1 count; ring not hidden) each fail `the_weapon_panel_is_the_ps2_frames_on_kamino`.

## Notes
Filed 2026-10-04 by Claude in chat from the maintainer's playtest (build 1bde70c, main 4e51e84). The maintainer's own words are quoted; nothing here is measured unless it says so. Related: BHUD3 (the BF2 HUD's grenade slot), AMMO (the infinite-ammo count).
