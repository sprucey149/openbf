---
id: BHUD3
title: The BF2 HUD, part 3 — the rest of the player's own elements: the grenade slot (needs a secondary weapon in the sim), the four side-LVL HUD meshes, the weapon name line (Locl), heat ring + overheat line, hit flashes, the jet bar, the target's name and health, the round timer and banner in BF2's own elements
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
BHUD2 (landed ea3883c, report reports/20261002_0125_card_BHUD2.md) draws health, stamina, weapon 1 + ammo + clip ring, the
reinforcement counts and the reticle from BF2's own tree. The interpreter already parses the elements below; they are
hidden only because nothing publishes their events, or their art is not baked.

## Do (each is an event feed in ui::hud::feed plus, where named, data)
- Side HUD meshes: add to rebake.sh's side group `bake weapon <SIDE>.LVL hud_cis_trishot|hud_wampa|hud_stealth|hud_imp_flamethrower side_bf2`
  (CIS / SNW / ALL / IMP) through `glb::hud_model_to_glb` if they carry COL0; today a weapon whose key maps to one of them
  shows no icon and no backdrop.
- Weapon name: `player1.weapon1.name` is sent empty (no Locl table in the port); EVMSG's Locl work gives the string.
- Heat: `.weapon1.heat` (ring + NumberColor), `player1.weaponsOverheat` (the Text needs the localised key).
- Hit flashes: `.weapon1.target.hit` / `.hitCritical` + `.hitColor` from the sim's Hit events for slot 0.
- Target under the reticle: `.target.className`, `.target.healthFraction` from Sim::reticle_pick.
- Jet bar: `player1.jetFuelFraction` when a jet class exists in the sim.
- Grenades (weapon 2): needs a secondary weapon in the sim first — split to its own card if the sim work is not done.
- Health regen pulse (`healthRegenPulseRate`), the BarBitmap flash copy (re-hud 2.2), the round timer / banner (egui today).
- A BF1 map: view the IconTexture fallback (`bitmap:<sheet>`, ui::hud_draw::icon) on a BF1 weapon with no BF2 mesh; on a BF1
  map a same-named weapon resolves to the BF2 side GLB first (game::hud_feed mesh_files order) — decide by a picture.

## Acceptance (measurable)
- game/tests/hud_layout.rs gains one assertion per element fed (its box at 640x480 against a PS2 frame that shows it:
  a capture with the clip ring up, one overheated, one with a target under the reticle).
- tat1_hud.png moves only if the golden's model feeds a new element (declared; viewed). No sim, sweep or arena move.

- Planner 2026-10-02 09:30: EVMSG landed (9d4dade): the Locl table this card waited for is baked
  (`common_bf{1,2}/locl/english.ron`, `manifest::hud::HudLocl`; 2,053 / 4,717 keys), so `player1.weapon1.name` and the
  overheat text can be fed. The message box and the points pop-up are done there; the tree is drawn under the spawn
  screen now.

- HUDW 2026-10-05 (landed 692fa46): the weapon rows are per-row now (`game::hud_feed::HudView::update`): a BF1 class
  keys the table by its own name, an unplaced model gives way to the icon sheet, a weapon with neither shows the
  backdrop alone. Still open here, seen on the PS2 frames `lead_tmp/hudw/ps2/cis/`: the SBD's wrist blaster / tri-shot
  show the red `hud_cis_wristblaster` / `hud_cis_trishot` badges and the wrist rocket its own model in row 2 - the
  port shows an empty pill for all three (the side HUD meshes of this card's first bullet). Not checked: BF1's pistol
  on a BF1 map takes BF2's `imp_weap_inf_pistol` mesh (a render before HUDW's alpha fix looked wrong - view it).

## Notes
- 2026-10-13 06:42 (lane 2, card CPBAR's RE; re-capture-display.md 3.4): BF2's HUD is NOT all layout data. The pause menu switches TEN code-drawn HUD pieces on and off together (B2:385841-385850: FUN_00393900, FUN_00393028, FUN_0038e9c0, FUN_00391200, FUN_0037e6d8 = the capture icon, FUN_00389b48, FUN_0037df18, FUN_003835f0, FUN_0037ed58, FUN_00390d20). One is read (the capture icon, card CPBAR); the other nine are not - read them on a PS2 frame before calling the player's HUD complete. A layout search that finds no element is not an absence on screen.

## Planner note
Planner 2026-10-21 20:50: the heat ring + overheat line moved to card OVHT (the maintainer's 10-21 ask, both games); leave them out here.

Note from OVHT's RE 2026-10-22 (`analysis/re-heat-hud.md` 2, 5): two more uses of the reticle's rings for this card.
(1) A COUNTED charge weapon (MaxPressedTime > 0: bowcaster, mortar launcher, arc caster, detpack, remote-droid destruct)
fills the HEAT ring with its charge while the trigger is held: `FUN_003b3398` sends `.weapon1.heat` = +0xbc / +0xc0
(time held / MaxPressedTime) in weapon state 3 (B2:409966-409980; state 3 entered by vtable +0xd8 `FUN_0032ee30`).
(2) `.weapon1.refire` (the red bits) also carries a counted weapon's state-4 progress, 1 - +0xbc/+0xc0
(B2:409971-409992); OVHT feeds only the overheat case.
