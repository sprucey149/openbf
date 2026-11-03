---
id: SBLT
title: Soldier base lighting: the port's un-passed droids are darker than the PS2's at the same pose (BF1 KAS2 SBD luma mean 26 vs 54, BF2 KAM1 battle droid 39 vs 48) — find which term (ambient, sun d4, the gamma-space GS modulate, the texture) and fix
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
DSHN's RE measured the droids with the specular pass removed, at the same camera, in both the PS2 and the port. The
port's soldier body is darker: about half the PS2's luma on BF1 KAS2, and 20 % darker on BF2 KAM1. Soldiers are on
screen all the time. Once DSHN removes the chrome, the droids will read as too dark instead of too shiny.

## Evidence
- `analysis/re-spec-gloss.md` §4 and §8; frames in `engine/artifacts/lead_tmp/dshn/`.
- The no-pass variants: PS2 `ps2/frames/*__nospec*.png`, port `port/*_nospec.png`. Both are measured on the droid's
  own pixels (`ps2/measure_union.py`, `measure_port.py`).
  - BF1 KAS2 SBD, mean / p95 / p99: PS2 54.2 / 113.5 / 161.7, port 26.1 / 68.1 / 116.3.
  - BF2 KAM1 battle-droid body: PS2 47.5 / 75.1 / 98.3, port 38.6 / 65.6 / 78.2. The BF2 PS2 droid wears the orange
    skin (alpha differs, RGB similar); the port wears the base sheet.
- **Caveats, not yet separated:**
  - the port's pose is `UNIT_POSES[0]`, not the PS2's animation;
  - the soldier faces along the camera in the port, while the PS2's heading was not read;
  - KAS2 sets no `CharacterAmbientColor` (CAMB), so the port uses the map's AmbientColor hemisphere.

## Acceptance
- **Plant the soldier exactly:** read the PS2 soldier's heading from RAM, re-render the port at the same pose
  (`lead_tmp/dshn/tools/dshn_soldier.rs`), and compare the no-pass body.
- **Name the term that differs,** with RE or a patched PCSX2 state removing one light term at a time
  (memory: patched save states):
  - the ambient;
  - the sun and its d4 falloff;
  - the VU1's 4-light sum on soldiers;
  - gamma-space modulate vs the port's linear;
  - the texture tier.
- Then fix it, or close the card if it is a pose artefact.

## Notes
- Do not confuse this with DSHN (the specular pass weight). Land DSHN first; its PS2 frames are the reference here too.
- Planner 2026-09-29 19:05: value 3 → 4. Since DSHN landed, the droids' highlights match the PS2 (+5.73 vs +5.9), but the whole droid now reads ~2x darker than the PS2 (SBD mean/p95/p99 32.8/104.3/137.8 vs 60.1/131.0/189.1, `reports/20260929_1850_card_DSHN.md`). Player-visible, and it is the rest of the maintainer's droid complaint. Reverse: set value=3.

## Planner note (2026-09-30, M-ALPHA)
`alpha: yes` — the droids are 2x too dark in every Clone Wars battle (the 'no droid chrome' fix's side effect). Value 4 (kept). Showcase maps: BF1 NAB1, BF1 TAT1 (the port's `tat1`), BF1 YAV2, BF2 UTA1, BF2 MYG1, BF2 HOT1 (PLAN §Milestones M-ALPHA).

## Note (card B1DK's RE, 2026-10-05): likely the light sum
The port adds ambient + sun in LINEAR light where the PS2 adds them in display units (`analysis/re-kamino-decks.md` §2): a surface under
an ambient and a sun of one size comes out 1.3-1.4 x dark. Soldiers take the same sum (`LIGHT_CHARACTER`), and BF1
soldiers at a post also lack the post's omni (card B1LL). Re-measure this card's boxes after LSUM lands before any
RE here.

## Note (card LSUM, 2026-10-05)
- 2026-10-05 (card LSUM landed, bf263f8): the light sum is now made in display units and the ambient floor is gone (report `reports/20261005_1505_card_LSUM.md`). Soldiers take the new sum and are brighter on all 52 unit sheets; unmeasured against the PS2. Re-measure this card's boxes on main bf263f8 first.
- 2026-10-06 planner (from WADE's report, FINDINGS F67, low; no card was filed): in WADE's pairs the port's BF1 stormtrooper reads darker and bluer than the PS2's - helmet (85, 94, 97) against (121, 122, 122) at BF1 KAS2 (`engine/artifacts/pcsx2/ref/bf1_kas2/wade/pairs/s094.png`, port main 2560be8). A third sample for this card's question beside the two droids in the title, and the first on a white surface (it shows the hue as well as the level). Use the pair as one of the acceptance poses.

## RE result (2026-10-16, lane 2; `analysis/re-soldier-light.md` [O], docs 87ad3b9) -> build
- **The term:** BF1 soldiers take ambient word 0x69c578. On a map WITHOUT `CharacterAmbientColor` it stays at the mission
  reset's (128,128,128) (the HUD model draw sets the key's flag every frame, so `AmbientColor` never reaches it). RAM of
  every BF1 state agrees. The port uses `AmbientColor` instead: bf1_kas2 (78,87,91, also the blue cast of WADE's
  stormtrooper), bf1_nab1 (105), bf1_yav2 (100). Showcase maps NAB1 and YAV2.
- PS2 KAS2 SBD (no pass): 51.2; with the port's ambient value 38.8. Port (main c3bff2a, the SBD's idle clip): 28.1; with
  128: 43.8. Ambient-only terms match (36.2 / 34.5).
- **BF2 KAM1 is no longer darker** (PS2 55.2 / 57.4, port 62.2 after LSUM/B1DK): the BF2 half of the title is closed.
- Left after the fix (~7 levels on KAS2): the PS2 takes 2.6-7x more sun from overhead / in front - card SSUN.
- The soldier LUMI tint (a darkening, NAB1 / TAT1 / END1 / TAT2, BF2 END1 / KAS2) - card SLUMI.

## Acceptance (build, replaces the RE acceptance above)
1. A unit test in `bake/src/lighting.rs`: a BF1 sky without `CharacterAmbientColor` bakes `character_ambient` =
   linear(128/255) per channel; with the key, the key's value; BF2 stays `None`. Mutation-check it.
2. After the rebake: bf1_kas2 / bf1_nab1 / bf1_yav2 manifests read `character_ambient: Some((0.2158…, …))`; every other
   map's manifest byte-equal.
3. The KAS2 pose number: `lead_tmp/sblt/tools/port_kas2.sh` with `SBLT_CLIP=human_sbdroid_stand_idle_emote` (build the
   scratch example `lead_tmp/sblt/tools/sblt_soldier.rs` at the branch, WITHOUT the `vtx_sun_shader.diff` scratch
   shader) -> the no-pass SBD mean on the fixed mask (`tools/measure_fixed.py port/kas2 180,220,460,480 ...`) reads
   43.8 ± 1.5 (was 28.1; PS2 51.2, the rest is SSUN's).

## Build (2026-10-22 07:01, lane 1) - built, PARKED on the RTX
- Branch `sblt` c7347aa: `bake::lighting::character_ambient_from` - a BF1 sky without `CharacterAmbientColor` bakes
  scale255(128,128,128); with the key the key; BF2 unchanged (None without it).
- Acceptance 1: `bf1_character_ambient_defaults_to_the_mission_reset`; mutation-checked twice (no default -> fails at
  the 0.2158 check; the default on BF2 too -> fails at the BF2 None check).
- Acceptance 2: every map group re-baked (all groups in gates2): SAME=23172 DIFF=3 = bf1_kas2 / bf1_nab1 / bf1_yav2
  `manifest.ron`, one line each `character_ambient: Some((0.21586053, x3))`.
- Acceptance 3 (Intel iGPU, the RTX is lost): no-pass SBD mean on the fixed mask 28.6 (main's bake) -> **44.5** (branch
  overlay; target 43.8 +- 1.5); the branch frame is byte-equal to main + `SBLT_CAMB=128`; mean RGB (25.2, 29.9, 31.0) ->
  (42.7, 45.2, 45.0): the blue cast is gone. Sheet `lead_tmp/sblt/sheet_build_kas2.png` (main | branch | PS2).
