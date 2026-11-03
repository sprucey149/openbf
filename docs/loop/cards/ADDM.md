---
id: ADDM
title: Additive model segments: the PS2 draws BF1 Normal 0x04|0x80 and BF2 0x04|0x40 segments ADDITIVE (GS 0x48, depth tested, no depth write); the port draws the 30 baked ones OPAQUE (17, their alpha dropped by ALPHA RULE v2 rule 2) or alpha-BLENDED (13) — DEA1's laser beams, FEL1's fel1_bldg_segmented, MUS1's droid concept, POL1's comm screens, KAM1/MYG1's carried CTF items
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Found by FXMF (2026-09-30, `analysis/re-muzzle-flash.md` §4.3): the muzzle-flash model's segment is ADDITIVE on the PS2 and
the bake stored its texture opaque. The GS rule is known [V]: `analysis/re-material-flags.md` §0.4/§2 — BF1 Normal class, mode
2 (0x04 without 0x02) blends, 0x80 picks ALPHA 0x48 (Cs·As + Cd); BF2 0x04 (or colour alpha ≠ 0xff) blends, 0x40 picks 0x48;
TEST 0x51001 (no Z write). ALPHA RULE v2 counts 0x40/0x80 as "modulation" bits, so such a texture's alpha — often the whole
shape of a glow or beam — is dropped (rule 2 → OPAQUE) or used as a plain lerp (BLEND). The port has no additive model pass.
## Evidence
Census of every baked GLB material's extras (FXMF session, one Python pass over `engine/assets_baked/**/*.glb`):
BF2 0x04|0x40: 29 materials — 15 OPAQUE rule 2 (com_sfx_lightningball1-3, geo_blast_ring, mus_sfx_droidconcept,
kam_icon_embryo_carried 0xc5, com_sfx_muzzleflash, …), 12 BLEND rule 2 (dea_sfx_laserbeam_{sm,md,lg}* 0x1010044,
pol1_prop_commscreen_scroll 0x1000045, com_prop_ctfdrop's spacedust1), 1 BLEND rule 1 (fel1_bldg_segmented1 0x54),
1 OPAQUE (myg_icon_powercrystal_carried); BF1 0x84 Normal: 1 (muzzleflash). Named in the map manifests: DEA1 dea_sfx_laserbeam
×27, FEL1 fel1_bldg_segmented ×74, MUS1 mus_sfx_droidconcept ×10, POL1 pol1_prop_commscreen_scroll ×8, KAM1 embryo ×4,
MYG1 power crystal ×4 (counts of names in manifest.ron, not yet split into placed instances vs classes).
## Acceptance
- A bake data test lists the additive segments by the ELF rule (BF1 Normal 0x04|0x80 without 0x02/0x20/0x400/0x800/0x4000;
  BF2 0x04|0x40) — today 30 — and each keeps its texture's disc alpha and an `additive` mode in the GLB extras.
- The port draws them through FXMF's additive model pass (depth tested, no depth write).
- PCSX2 walkabout pairs of DEA1 / FEL1 / MUS1 / POL1 at posts where these props are in view, viewed before and after;
  `scripts/parity.py` does not rise on any map (declare parity= for the ones that move).
- FXMF build (2026-09-30, lane 2): the flash's additive draw is `render::ParticleShape::Mesh` — a small model transformed
  on the CPU into the particle list (ADDITIVE 0x48, depth tested, no write, unfogged, sorted with bolts/particles). Right
  for effect models (FXP4's GEOMETRY), NOT for world segments: ADDM still wants a GPU `DrawLayer` for static instanced
  models. The flash sheet keeps its alpha through `bake::alpha::flash_textures` (a named-consumer clause), not a rule change.
