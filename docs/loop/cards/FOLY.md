---
id: FOLY
title: Soldier foley: footsteps by surface (run / walk, left / right), jump, land, roll, squat, get up - the PS2 starts 237 in 105 s on Naboo, the port none
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
SNDC (`analysis/audio-census.md` sections 2, 3): the PS2 plays a footstep for every step of every soldier in earshot,
and jump / land / roll / squat / get-up sounds. The port plays none. In 105 s on NAB1 the PS2 started 237 foley
sounds (217 steps); the player's own are the most constant sound of the game after his gun.

## Evidence (live PCSX2, `lead_tmp/sndc/`, `tab.py <run> <game> <P> near`)
- Names (hash-only on the disc, recovered): `<side>_inf_<unit>_<action>_<surface>[_left|_right]` - units `droid`,
  `trooper`, `soldier`; actions `run`, `walk` (x left / right), `jump`, `land`, `roll`, `squat`, `lie`, `getup`,
  `stop`; surfaces heard: `grass` (NAB1), `stone`, `metal` (UTA1); `water`, ... exist. Template 0xFE30AE3A: gain 0.7,
  10 - 50 m, rolloff 3; four takes per step property.
- Cadence of the player (super battle droid): run steps alternate right / left every 0.30 s on NAB1, 0.27 s on UTA1;
  a `walk` step at the start and the end of a run; `jump` on the press, `land` 0.90 - 1.03 s later; BF2 Circle ->
  `squat` (standing) / `roll` (moving).
- The per-WEAPON ODF keys `JumpSound` / `LandSound` / `RollSound` / `SquatSound` / `ProneSound` / `StandSound`
  (`*_weap_inf_*_mvt_*`, 31 classes BF1, 93-100 BF2) were NOT heard in either run.
- ELF: `FoleyFXCollider`, `FoleyFXCollidee` classes (strings `15FoleyFXCollider`, `20FoleyFXColliderClass`).

## RE - DONE 2026-10-02 (`analysis/re-foley.md`, `[O]`, verifier 48 / 16 PARTLY / 0 of 64, folded in)
- The table: `ffx_` chunks in `SOUND/<PLANET>.LVL`, per sub-level `<map>` + `cw` / `gcw`: `FoleyFXGroup`
  (`terrain_foley`, `stone_foley`, `metal_foley`, ...) -> `FoleyFXSoldier` rows (a `Class` + 13 sound slots, BF2 11).
- The row: the soldier class's ODF `FoleyFXClass`. The surface: the last body touched - terrain -> `terrain_foley`,
  under water -> `water_foley`, a prop -> its class's `FoleyFXGroup`; a group with no row for the class keeps the last.
- A step: the DRAWN clip's foot lifting off again (foot bone dropped, then rises from below 0.17 m). 2.0 m per run
  step. Only a drawn soldier steps; an AI soldier off screen makes none. `FootstepStride` 1.75 m is first person only.
- Walk / run by the clip: BF1 forward > 5.5 m/s (5.0 to stay), backward and strafe LEFT always run, strafe right
  > 5.0 / 5.5; BF2 > 0.78 x MaxSpeed (0.71 to stay).
- The weapon `*_mvt_*` keys: started by the code, no sample on either disc -> play nothing. Stop slots: never read.

## Original Do / Acceptance (2026-10-02, before the RE)
RE: which foley group a soldier class uses and where the table is on the disc (the `ffx` data), how the surface is
chosen (terrain texture, collision material), what starts a step (an animation event, a distance) and the walk / run
threshold, when the weapon `*_mvt_*` keys play. Then the bake (the table) and the play (`game::event_sounds`).
- RE: `analysis/re-foley.md` with the five checks and the verifier.
- Build, on the SOUND_LOG referee (`lead_tmp/sndc/port/sound_log.patch`, `pad120.ron`): the player's run on bf1_nab1
  logs `cis_inf_droid_run_grass_right` / `_left` alternating every 0.30 +- 0.03 s, the jump `..._jump_grass` then
  `..._land_grass`; on bf2_uta1 the `_stone` set at 0.27 +- 0.03 s; total steps in range within 2x of the PS2's 217
  (NAB1) per 105 s.
