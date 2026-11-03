---
id: UWPN
title: The battle draws each class's own gun: SoldierVisuals caches one asset per body model, so a class sharing a model with an earlier class draws that class's weapon (BF1 sharpshooter holds a rifle, assassin and pilot droids hold the assault droid's launcher, snow shock/scout/vanguard hold rifles)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
UAUD1 (analysis/unit-audit-bf1.md §3.2), measured by running the game's own roster over the 26 BF1 mission RONs
(`engine/artifacts/lead_tmp/unit-audit/port_roster_bf1.txt`, the MISMATCH lines): `SoldierVisuals::load` keys its
cache on `<game>/<GeometryName>` (game/src/soldier_vis.rs 128-131), the cached asset carries the FIRST class's hand
weapon (`load_one` 216-250) and `weapon_draws` draws `sa.weapon` (386-395).

## Evidence
| class | draws | should draw | missions |
|---|---|---|---|
| rep_inf_clone_sharpshooter | rep_weap_inf_rifle | rep_weap_inf_sniperrifle | 13 CW |
| cis_inf_pilotdroid | cis_weap_inf_launcher | com_weap_inf_torpedo | 13 CW |
| cis_inf_assassindroid | cis_weap_inf_launcher | cis_weap_inf_sniperrifle | 13 CW |
| imp_inf_shock_troopersnow | imp_weap_inf_rifle | imp_weap_inf_launcher | hot1, rhn1, rhn2 |
| imp_inf_scout_troopersnow | imp_weap_inf_rifle | imp_weap_inf_sniperrifle | hot1, rhn1, rhn2 |
| all_inf_vanguardsnow | all_weap_inf_rifle | all_weap_inf_launcher | hot1, rhn1, rhn2 |
The PS2's class-select screens show each of these classes with its own gun (`lead_tmp/unit-audit/pcsx2/*_sheet.png`).

## Notes
- UGAL fixes the unit GALLERY's weapon guess; this is the battle's own path. Both should use the roster's
  `weapon_geometry`.
- UAUD2 (2026-09-29, analysis/unit-audit-bf2.md §3.2): BF2 has the same bug on 8 classes (9 class-gun pairs) over the 31 BF2 RONs:
  `cis_inf_engineer` and `cis_inf_sniper` draw the assault droid's `cis_weap_inf_launcher` on all 15 CW RONs (the PS2
  shows each with its own gun, `lead_tmp/unit-audit/pcsx2_bf2/cis_droid_skins.png`); hot1's `all_inf_rocketeer_snow`,
  `imp_inf_rocketeer_snow`, `imp_inf_sniper_snow` draw rifles; `all_inf_rocketeer_fleet` (dea1, tan1) and
  `all_inf_sniper_fleet` (dea1, tan1: rifle; kam1: launcher); geo1's `rep_inf_ep2_sniper` draws the rifle.
  Measured with `lead_tmp/unit-audit/uaud2_roster.rs` -> `port_roster_bf2.txt` (MISMATCH lines).

## Landed (2026-10-02, 2e54da7) — report `reports/20261002_1445_card_UWPN.md`
Bodies and hand weapons are cached apart (`soldier_vis::DrawPlan`). Acceptance run: `roster::battle_classes_hold_their_own_weapon_model`
over every mission RON — class slots drawn with another class's gun: BF1 48 -> 0 (of 247), BF2 39 -> 0 (of 357);
`soldier_vis::tests::classes_sharing_a_body_hold_their_own_guns`; both fail with the old rule put back. Battle frames
viewed (BF1 NAB1 CW, before / after): `lead_tmp/uwpn/frames/rep_pair.png`, `cis_pair.png`. No golden moved.
