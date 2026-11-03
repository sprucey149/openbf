---
id: DSHN
title: Droids look like polished chrome (the maintainer's play session 2026-09-29): the SBD and the other droids take hard white highlights in the port where the PS2's are matte grey — RE the skinned specular pass per droid class (colour, power, whether the PS2 gives droid bodies a specular pass at all), check CAMB's effect, then build
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer's first play session (INBOX 2026-09-29, value 5, front of the queue): the super battle droid looks like polished
chrome, with hard white highlights. The maintainer says the other droids look similarly shiny. Droids are half of every Clone Wars
battle, so the player sees this constantly.

## Evidence
- Port: `lead_tmp/playtest_0929/shiny_sbd.png` (BF1 NAB-style ground, SBD in play: white bands on the arms, chest and legs).
- PS2: `lead_tmp/unit-audit/pcsx2/sbd_ingame.png`, `10_sbd_idle.png`, `10_sbd_idle_crop.png` (UAUD1's frames). The PS2's
  SBD is a matte mid grey, with one soft highlight on the head dome. (the maintainer's note gives these at `pcsx2/`; they are
  in `lead_tmp/unit-audit/pcsx2/`.)
- Suspects (the maintainer's leads, to be proven or refuted, not assumed):
  - the skinned specular second pass: `engine/crates/game/src/soldier_vis.rs` (`base.specular(rgb, power)`,
    `DrawCmd::specular`, queue SPEC2), `analysis/re-specular-pass.md`. Check the per-part specular colour and power
    against the disc's material flags, and whether the PS2 runs a specular pass on droid bodies at all;
  - CAMB (e383c90 + goldens 86d23b5, 09-28): `LightPath::Character` also lights the held weapon's base + specular draws.
    Render the SBD before and after CAMB to see whether it got shinier then or was always this way.
  - Prior art: `lead_tmp/spec-side/` and `lead_tmp/two-tier-soldier/` hold earlier SBD sheets (before/after renders).

## Notes
- RE at max effort (the RE discipline + the foreground verifier); then phase=build with files/declares.
- To capture a droid in the same lighting: `plant-a-soldier-with-a-scratch-example` (port) and the PCSX2 RAM hold
  (`ps2-camera-anywhere-by-ram-hold`); BF1 SBD frames exist from UAUD1; BF2 via `lead_tmp/unit-audit/*2.py` states.

## RE answer (2026-09-29, lane 2; `analysis/re-spec-gloss.md`, commit 816a9e1, [O])
- **Cause:** the PS2 weights the specular add by Ad, the base texel's alpha, which is a dark gloss mask on every droid
  sheet (SBD mean 34/255). The port bakes those sheets at alpha 255, so `fs_spec` adds at weight 1.0. Colour, power,
  table and tex-gen are not the defect; CAMB is not either (chrome since SPEC2 `c231edb`).
- **Proven on the PS2:** a patched record (ALPHA C → FIX 0x80 = weight 1.0) turns the PS2's own SBD and battle droid
  chrome (`lead_tmp/dshn/DSHN_pairs.png`).
- **Targets, luma mean / p95 / p99 on the droid's pixels** (the PS2 as-is; weight 1.0; port now):
  - BF1 KAS2 SBD: 60.1 / 131.0 / 189.1; 100.1 / 221.7 / 243.9; port now 67.2 / 217.1 / 243.6.
  - BF2 KAM1 battle droid: 52.0 / 80.8 / 100.5; 86.7 / 199.2 / 226.4; port now 70.0 / 171.6 / 248.5.
- **Port prototype** (the disc alpha restored): the add matches the PS2's (+5.5 vs +5.9; +4.9 vs +4.5).
- **Troopers and clones:** no body pass on either disc, so no regression is possible. Their rifles' sheets are LOST
  too and will dim toward the PS2.
- **Also in scope:** 68 sheets in all (every 1st-person weapon, 29 world sheets: KAM1 decks, DEA1, UTA1…).
- **Separate:** the port's un-passed droid base is darker than the PS2's → card SBLT.
- **Override skins:** the droid classes' skins (assassin/pilot/…) are USKN's; they need the same keep once baked.
- **AUDIT 2026-09-29 (lane 1): OK** (`audits.md`; banner 289ab49 in `analysis/re-spec-gloss.md`).
  - The RE's records, census and SBD numbers reproduce independently. So does the BF1 port prototype: the disc alpha
    restored, add +5.73 (the RE's +5.5), base unchanged.
  - The PS2 itself weights a WORLD sheet by its gloss alpha too. On the BF1 KAM1 deck, the per-pixel weight's
    distribution equals `kam1_bldg_plat1`'s disc alpha (`lead_tmp/audit-dshn/kam1post2_deck_crop.png`), so the 29
    world sheets belong in the build.
  - All 9 lost sheets that have a `.tier2.ktx2` are alpha 255 there too (BF2 SBD, battle droid, magnaguard, the rifle,
    and 5 world sheets: 2 DEA1 rooms, FEL1's bladder, NAB2's fountain, UTA1's generator), so `take_alpha_from` must cover them.
  - Patched KAM1/KAS2 states, frames and `weight.py` (the PS2 weight map) are in `lead_tmp/audit-dshn/`, for the
    acceptance pairs.
