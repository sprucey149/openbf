---
id: WHELD
title: The held weapon model follows the selected weapon: a soldier (the player and the bots) draws the weapon in its hands, not always its class's primary
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/game/src/soldier_vis.rs, engine/crates/game/src/roster.rs, engine/crates/game/src/walk.rs, engine/crates/game/examples/round_to_outcome.rs, scripts/rebake.sh
declares: rebake=side
needs:
effort:
branch: wheld
resume: LANDED e95a687
created: 2026-10-19
first: 3
---

## Why
(the maintainer, 2026-10-19) Switching weapons in the game changes how the soldier fires and behaves, but the soldier still holds the model of its class's primary: `game/src/soldier_vis.rs` `weapon_draws` (line ~1084) draws `weapon_for(team, class)`, the class's primary, for every soldier; only the animator follows the weapon in hand. The held model must follow the selected weapon, for the player and for the bots.

## Evidence
`soldier_vis.rs` header comments (primary-weapon GeometryNames in first-use order, `weapon_for`); the fire point is read off the drawn weapon's FirePointName node (`soldier_vis` fire-point function), so it must follow the same model; card HUDW baked BF2's pistols for the HUD row; `analysis/re-grenades.md` §2.4 (the weapon channels and SWITCH_WEAPON); the PS2's own behaviour for what a grenade or a secondary looks like in the hand needs a PCSX2 look (re-soldier-anim-layers.md for the hold poses).

## Notes
Reads sim state, writes none. If the animator's hold pose per weapon is wrong for a model, that is ANIMX's, not this card's.

## Build result (2026-10-20, branch `wheld`, a14ae74 + bef41b0)
The PS2 rule (re-muzzle-flash.md §5 [V][O], re-grenades.md §2.3 item 4 and §3.2): the soldier render draws the ACTIVE
weapon (soldier+0xbdc = cur[active channel]) at `hp_weapons`; a grenade with `HideOnFire` sets weapon+0xbc at its fire
entry (not drawn) until its reload exit. Port: `soldier_vis::weapon_in_hand` (channel 1's current while active_channel
1, else `Unit::weapon_slot`), `DrawPlan::weapon_in_slot`; the fire point and the flash look follow it.

Acceptance, as run:
1. Census `game/tests/suite/roster.rs every_held_slots_hand_model_is_baked` (-- --nocapture prints every slot; full
   rows `lead_tmp/wheld/census.txt`): 83 fielded classes; slots the sim runs: 216 with a baked model, 4 with none (BF1
   battledroid wrist blaster / shrapnel, BF2 rifleman wrist blaster / trishot: no GeometryName, nothing drawn - as the
   PS2, re-muzzle-flash §4.1), 0 unbaked. Before the rebake: 97 unbaked (BF1 four pistols, com_weap_inf_grenadethermal).
2. `rebake.sh side` adds BF1 all/imp/rep/cis_weap_inf_pistol + com_weap_inf_grenadethermal (BF1 COMMON, BF2 INGAME):
   the gates' bake row ONLY_IN_A=20 files, DIFF=0. Not baked: detpacks, fusion cutters, mines, remote droids - slots the
   sim does not run yet (MINE / UORD bake them).
3. `soldier_vis::tests::the_weapon_in_hand_follows_the_switch_and_the_throw` (sim pad path: switch -> pistol slot 1;
   throw -> thermal hidden; then pistol; on a baked BF2 clone the fire point = the pistol's hp_fire under the hand, not
   the rifle's) and `a_slot_in_hand_draws_its_own_model`; two mutants caught (spec slot instead of the swapped one;
   fire point off slot 0). Battle: `ANIM_CENSUS=1 round_to_outcome 9000 3 <map>` `hands:` lines - BF2 KAM1 bots with
   a non-primary in hand 37,396 of 262,176 unit-ticks (pistols 30,633, mortar launcher as com_weap_inf_torpedo 6,763),
   BF1 NAB1 18,579 of 199,052; grenades: hidden in every channel-1 tick (262 / 1,669). Picture: one body, two weapons
   `lead_tmp/wheld/pics/bf{1,2}_*_{rifle,pistol}.png` (gallery `unit_sheet`).
4. Gates: goldens unchanged (no golden view holds a non-primary), sim / simhash / sweep / arena identical.

### Census by weapon (game, weapon, channel, hand model, baked after the rebake, sim runs it, classes)
| game | weapon | ch | hand model | baked | sim runs it | classes |
|---|---|---|---|---|---|---|
| bf1 | all_weap_inf_bowcaster | 0 | all_weap_inf_bowcaster | yes | yes | 2 |
| bf1 | all_weap_inf_crackgrenade | 1 | com_weap_inf_grenadethermal | yes | yes | 5 |
| bf1 | all_weap_inf_fusioncutter | 1 | all_weap_inf_fusioncutter | NO | no | 1 |
| bf1 | all_weap_inf_mine_dispenser | 1 | com_weap_inf_landmine | NO | no | 3 |
| bf1 | all_weap_inf_mortar_launcher | 0 | com_weap_inf_torpedo | yes | yes | 2 |
| bf1 | all_weap_inf_pistol | 0 | all_weap_inf_pistol | yes | yes | 12 |
| bf1 | all_weap_inf_powerup_dispenser | 1 | - | - | no | 1 |
| bf1 | all_weap_inf_remote_charge_dispenser | 1 | com_weap_inf_seismic_mine | NO | no | 2 |
| bf1 | all_weap_inf_remotedroid | 1 | com_weap_inf_grenadethermal_ord | NO | no | 3 |
| bf1 | all_weap_inf_rifle | 0 | all_weap_inf_rifle | yes | yes | 5 |
| bf1 | all_weap_inf_shotgun | 0 | all_weap_inf_seeker | yes | yes | 1 |
| bf1 | all_weap_inf_sniperrifle | 0 | all_weap_inf_sniperrifle | yes | yes | 3 |
| bf1 | all_weap_inf_thermaldetonator | 1 | com_weap_inf_grenadethermal | yes | yes | 11 |
| bf1 | all_weap_inf_torpedo_launcher | 0 | all_weap_inf_launcher | yes | yes | 3 |
| bf1 | cis_weap_inf_fusioncutter | 1 | cis_weap_inf_fusioncutter | NO | no | 1 |
| bf1 | cis_weap_inf_mine_dispenser | 1 | - | - | no | 1 |
| bf1 | cis_weap_inf_mortar_launcher | 0 | com_weap_inf_torpedo | yes | yes | 1 |
| bf1 | cis_weap_inf_pistol | 0 | cis_weap_inf_pistol | yes | yes | 3 |
| bf1 | cis_weap_inf_powerup_dispenser | 1 | - | - | no | 1 |
| bf1 | cis_weap_inf_remotedroid | 1 | com_weap_inf_grenadethermal_ord | NO | no | 1 |
| bf1 | cis_weap_inf_sniperrifle | 0 | cis_weap_inf_sniperrifle | yes | yes | 1 |
| bf1 | cis_weap_inf_thermaldetonator | 1 | com_weap_inf_grenadethermal | yes | yes | 2 |
| bf1 | cis_weap_inf_torpedo_launcher | 0 | cis_weap_inf_launcher | yes | yes | 1 |
| bf1 | cis_weap_inf_wrist_blaster | 0 | - | - | yes | 1 |
| bf1 | cis_weap_inf_wrist_rocket | 1 | - | - | no | 1 |
| bf1 | cis_weap_inf_wrist_shrapnel | 0 | - | - | yes | 1 |
| bf1 | imp_weap_inf_crackgrenade | 1 | com_weap_inf_grenadethermal | yes | yes | 2 |
| bf1 | imp_weap_inf_fusioncutter | 1 | imp_weap_inf_fusioncutter | NO | no | 3 |
| bf1 | imp_weap_inf_mine_dispenser | 1 | - | - | no | 2 |
| bf1 | imp_weap_inf_mortar_launcher | 0 | com_weap_inf_torpedo | yes | yes | 3 |
| bf1 | imp_weap_inf_pistol | 0 | imp_weap_inf_pistol | yes | yes | 11 |
| bf1 | imp_weap_inf_powerup_dispenser | 1 | - | - | no | 3 |
| bf1 | imp_weap_inf_remotedroid | 1 | com_weap_inf_grenadethermal_ord | NO | no | 2 |
| bf1 | imp_weap_inf_rifle | 0 | imp_weap_inf_rifle | yes | yes | 2 |
| bf1 | imp_weap_inf_shotgun | 0 | imp_weap_inf_rifle | yes | yes | 2 |
| bf1 | imp_weap_inf_sniperrifle | 0 | imp_weap_inf_sniperrifle | yes | yes | 2 |
| bf1 | imp_weap_inf_thermaldetonator | 1 | com_weap_inf_grenadethermal | yes | yes | 8 |
| bf1 | imp_weap_inf_torpedo_launcher | 0 | imp_weap_inf_launcher | yes | yes | 2 |
| bf1 | rep_weap_inf_arccaster | 0 | rep_weap_inf_rifle | yes | yes | 1 |
| bf1 | rep_weap_inf_commando_pistol | 0 | rep_weap_inf_pistol | yes | yes | 1 |
| bf1 | rep_weap_inf_crackgrenade | 1 | com_weap_inf_grenadethermal | yes | yes | 1 |
| bf1 | rep_weap_inf_fusioncutter | 1 | rep_weap_inf_fusioncutter | NO | no | 1 |
| bf1 | rep_weap_inf_haywiredetonator | 1 | com_weap_inf_grenadethermal | yes | yes | 1 |
| bf1 | rep_weap_inf_mine_dispenser | 1 | rep_weap_inf_landmine | NO | no | 1 |
| bf1 | rep_weap_inf_pistol | 0 | rep_weap_inf_pistol | yes | yes | 4 |
| bf1 | rep_weap_inf_powerup_dispenser | 1 | - | - | no | 1 |
| bf1 | rep_weap_inf_remotedroid | 1 | - | - | no | 1 |
| bf1 | rep_weap_inf_rifle | 0 | rep_weap_inf_rifle | yes | yes | 1 |
| bf1 | rep_weap_inf_rocket_launcher | 0 | com_weap_inf_torpedo | yes | yes | 1 |
| bf1 | rep_weap_inf_sniperrifle | 0 | rep_weap_inf_sniperrifle | yes | yes | 1 |
| bf1 | rep_weap_inf_thermaldetonator | 1 | com_weap_inf_grenadethermal | yes | yes | 3 |
| bf1 | rep_weap_inf_torpedo_launcher | 0 | rep_weap_inf_launcher | yes | yes | 1 |
| bf2 | all_weap_award_pistol | 0 | all_weap_inf_pistol | yes | no | 13 |
| bf2 | all_weap_award_rifle | 0 | all_weap_inf_rifle | yes | no | 5 |
| bf2 | all_weap_award_rocket_launcher | 0 | all_weap_inf_launcher | yes | no | 4 |
| bf2 | all_weap_award_shotgun | 0 | all_weap_inf_seeker | yes | no | 4 |
| bf2 | all_weap_award_sniper_rifle | 0 | all_weap_inf_sniperrifle | yes | no | 4 |
| bf2 | all_weap_inf_autoturret_dispenser | 1 | - | - | no | 4 |
| bf2 | all_weap_inf_bowcaster | 0 | all_weap_inf_bowcaster | yes | yes | 2 |
| bf2 | all_weap_inf_buff_health | 1 | - | - | no | 3 |
| bf2 | all_weap_inf_detpack | 1 | com_weap_inf_detpack | NO | no | 4 |
| bf2 | all_weap_inf_fusioncutter | 0 | all_weap_inf_fusioncutter | NO | no | 4 |
| bf2 | all_weap_inf_incinerator | 0 | all_weap_inf_seeker | yes | yes | 3 |
| bf2 | all_weap_inf_invisibility | 0 | - | - | no | 3 |
| bf2 | all_weap_inf_mine_dispenser | 1 | - | - | no | 4 |
| bf2 | all_weap_inf_mortar_launcher | 0 | com_weap_inf_torpedo | yes | yes | 2 |
| bf2 | all_weap_inf_pistol | 0 | all_weap_inf_pistol | yes | yes | 13 |
| bf2 | all_weap_inf_powerup_dispenser | 1 | - | - | no | 4 |
| bf2 | all_weap_inf_remotedroid | 1 | com_weap_inf_grenadethermal_ord | NO | no | 2 |
| bf2 | all_weap_inf_rifle | 0 | all_weap_inf_rifle | yes | yes | 5 |
| bf2 | all_weap_inf_rocket_launcher | 0 | all_weap_inf_launcher | yes | yes | 4 |
| bf2 | all_weap_inf_shotgun | 0 | all_weap_inf_seeker | yes | yes | 4 |
| bf2 | all_weap_inf_sniper_rifle | 0 | all_weap_inf_sniperrifle | yes | yes | 4 |
| bf2 | all_weap_inf_thermaldetonator | 1 | com_weap_inf_grenadethermal | yes | yes | 15 |
| bf2 | all_weap_inf_timebomb | 1 | - | - | no | 3 |
| bf2 | cis_weap_award_pistol | 0 | cis_weap_inf_pistol | yes | no | 2 |
| bf2 | cis_weap_award_rocket_launcher | 0 | cis_weap_inf_launcher | yes | no | 1 |
| bf2 | cis_weap_award_shotgun | 0 | cis_weap_inf_rifle | yes | no | 1 |
| bf2 | cis_weap_award_sniper_rifle | 0 | cis_weap_inf_sniperrifle | yes | no | 1 |
| bf2 | cis_weap_award_wrist_blaster | 0 | - | - | no | 1 |
| bf2 | cis_weap_inf_autoturret_dispenser | 1 | - | - | no | 1 |
| bf2 | cis_weap_inf_bulldog | 0 | cis_weap_inf_rifle | yes | yes | 1 |
| bf2 | cis_weap_inf_debuff_poison | 1 | - | - | no | 1 |
| bf2 | cis_weap_inf_detpack | 1 | com_weap_inf_detpack | NO | no | 1 |
| bf2 | cis_weap_inf_fusioncutter | 0 | cis_weap_inf_fusioncutter | NO | no | 1 |
| bf2 | cis_weap_inf_mine_dispenser | 1 | - | - | no | 1 |
| bf2 | cis_weap_inf_mortar_launcher | 0 | com_weap_inf_torpedo | yes | yes | 1 |
| bf2 | cis_weap_inf_pistol | 0 | cis_weap_inf_pistol | yes | yes | 2 |
| bf2 | cis_weap_inf_powerup_dispenser | 1 | - | - | no | 1 |
| bf2 | cis_weap_inf_remotedroid | 1 | com_weap_inf_grenadethermal_ord | NO | no | 1 |
| bf2 | cis_weap_inf_rocket_launcher | 0 | cis_weap_inf_launcher | yes | yes | 1 |
| bf2 | cis_weap_inf_shotgun | 0 | cis_weap_inf_rifle | yes | yes | 1 |
| bf2 | cis_weap_inf_sniper_rifle | 0 | cis_weap_inf_sniperrifle | yes | yes | 1 |
| bf2 | cis_weap_inf_thermaldetonator | 1 | com_weap_inf_grenadethermal | yes | yes | 2 |
| bf2 | cis_weap_inf_wrist_blaster | 0 | - | - | yes | 1 |
| bf2 | cis_weap_inf_wrist_rocket | 1 | - | - | no | 1 |
| bf2 | cis_weap_inf_wrist_trishot | 0 | - | - | yes | 1 |
| bf2 | imp_weap_award_pistol | 0 | imp_weap_inf_pistol | yes | no | 7 |
| bf2 | imp_weap_award_rifle | 0 | imp_weap_inf_rifle | yes | no | 2 |
| bf2 | imp_weap_award_rocket_launcher | 0 | imp_weap_inf_launcher | yes | no | 2 |
| bf2 | imp_weap_award_shotgun | 0 | imp_weap_inf_rifle | yes | no | 2 |
| bf2 | imp_weap_award_sniper_rifle | 0 | imp_weap_inf_sniperrifle | yes | no | 2 |
| bf2 | imp_weap_inf_arccaster | 0 | imp_weap_inf_rifle | yes | yes | 1 |
| bf2 | imp_weap_inf_autoturret_dispenser | 1 | - | - | no | 2 |
| bf2 | imp_weap_inf_buff_damage | 1 | - | - | no | 1 |
| bf2 | imp_weap_inf_commando_pistol | 0 | imp_weap_inf_pistol | yes | yes | 1 |
| bf2 | imp_weap_inf_detpack | 1 | com_weap_inf_detpack | NO | no | 2 |
| bf2 | imp_weap_inf_fusioncutter | 0 | imp_weap_inf_fusioncutter | NO | no | 2 |
| bf2 | imp_weap_inf_mine_dispenser | 1 | - | - | no | 2 |
| bf2 | imp_weap_inf_mortar_launcher | 0 | com_weap_inf_torpedo | yes | yes | 1 |
| bf2 | imp_weap_inf_pistol | 0 | imp_weap_inf_pistol | yes | yes | 6 |
| bf2 | imp_weap_inf_powerup_dispenser | 1 | - | - | no | 2 |
| bf2 | imp_weap_inf_remotedroid | 1 | com_weap_inf_grenadethermal_ord | NO | no | 1 |
| bf2 | imp_weap_inf_rifle | 0 | imp_weap_inf_rifle | yes | yes | 2 |
| bf2 | imp_weap_inf_rocket_launcher | 0 | imp_weap_inf_launcher | yes | yes | 2 |
| bf2 | imp_weap_inf_shotgun | 0 | imp_weap_inf_rifle | yes | yes | 2 |
| bf2 | imp_weap_inf_sniper_rifle | 0 | imp_weap_inf_sniperrifle | yes | yes | 2 |
| bf2 | imp_weap_inf_sonic_blaster | 0 | imp_weap_inf_pistol | yes | yes | 1 |
| bf2 | imp_weap_inf_thermaldetonator | 1 | com_weap_inf_grenadethermal | yes | yes | 7 |
| bf2 | rep_weap_award_pistol | 0 | rep_weap_inf_pistol | yes | no | 10 |
| bf2 | rep_weap_award_rifle | 0 | rep_weap_inf_rifle | yes | no | 2 |
| bf2 | rep_weap_award_rocket_launcher | 0 | rep_weap_inf_launcher | yes | no | 2 |
| bf2 | rep_weap_award_shotgun | 0 | rep_weap_inf_rifle | yes | no | 2 |
| bf2 | rep_weap_award_sniper_rifle | 0 | rep_weap_inf_sniperrifle | yes | no | 3 |
| bf2 | rep_weap_inf_autoturret_dispenser | 1 | - | - | no | 3 |
| bf2 | rep_weap_inf_buff_defense | 1 | - | - | no | 1 |
| bf2 | rep_weap_inf_chaingun | 0 | rep_weap_inf_chaingun | yes | yes | 1 |
| bf2 | rep_weap_inf_commando_pistol | 0 | rep_weap_inf_pistol | yes | yes | 2 |
| bf2 | rep_weap_inf_detpack | 1 | com_weap_inf_detpack | NO | no | 2 |
| bf2 | rep_weap_inf_emp_launcher | 0 | com_weap_inf_torpedo | yes | yes | 2 |
| bf2 | rep_weap_inf_fusioncutter | 0 | rep_weap_inf_fusioncutter | NO | no | 2 |
| bf2 | rep_weap_inf_mine_dispenser | 1 | - | - | no | 2 |
| bf2 | rep_weap_inf_pistol | 0 | rep_weap_inf_pistol | yes | yes | 8 |
| bf2 | rep_weap_inf_powerup_dispenser | 1 | - | - | no | 2 |
| bf2 | rep_weap_inf_remotedroid | 1 | com_weap_inf_grenadethermal_ord | NO | no | 1 |
| bf2 | rep_weap_inf_rifle | 0 | rep_weap_inf_rifle | yes | yes | 2 |
| bf2 | rep_weap_inf_rocket_launcher | 0 | rep_weap_inf_launcher | yes | yes | 2 |
| bf2 | rep_weap_inf_shotgun | 0 | rep_weap_inf_rifle | yes | yes | 2 |
| bf2 | rep_weap_inf_sniper_rifle | 0 | rep_weap_inf_sniperrifle | yes | yes | 3 |
| bf2 | rep_weap_inf_thermaldetonator | 1 | com_weap_inf_grenadethermal | yes | yes | 9 |
