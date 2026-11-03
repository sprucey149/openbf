---
id: KTEX
title: BF1 KAS2's slope behind Dock Two is pale sand on the PS2 and flat dark grass in the port
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why / Evidence / Acceptance
See the original entry below; the first session to work this card rewrites this section into short Why / Evidence / Acceptance lines and fills `files:` and `declares:`.

## Original QUEUE.md entry (verbatim, 2026-09-25)
KTEX. [TODO — **NOTE 2026-09-25 (WCOL's RE, `analysis/re-water-colour.md` §0/§2): BF1 KAS2's channel bed under Dock One's water is pale SAND on the PS2 (the water's alpha zeroed in a patched save state: box mean (107,96,77)) and dark GRASS in the port ((41,43,24)) — F6's family, and the largest gap left in F1's frame once the water is fixed; `engine/artifacts/lead_tmp/re/wcol/sheets/wcol_bf1_kas2.png`.** **NOTE 2026-09-25 (PXC batch 2, FINDINGS F14, F17, F21, F23): the census has four more maps — BF1 KAS1's ground is yellow-green grass and reeds on the PS2 and flat grey sand in the port (every pair, and the port's own walkabout sheet `engine/artifacts/walkabout/bf1_kas1/cp2_cp2.png`), BF1 KAS2's high ground green grass vs brown gravel (`engine/artifacts/pcsx2/ref/bf1_kas2/pairs/cp3_cp3_cityhall_out*.png`), BF1 YAV1's jungle floor darker and denser vs blotchy yellow-green (`…/bf1_yav1/pairs/cp2_viaduct_viaduct_spawn.png`), BF2 NAB2's plaza warm brown dirt vs cooler, lighter tiles (`…/bf2_nab2/pairs/cp4_cp6_postA_v1.png`, LOW-MEDIUM). Separate the layer colour from the missing FOLIAGE (queue FOLG) before attributing a pair.** ADDED 2026-09-25 (lane 1) by PXC batch 1 (FINDINGS F6); **[RE] FIRST**; FREE] **BF1 KAS2's slope behind Dock Two is pale sand on the PS2 and flat dark grass in the port** [RE] — evidence `engine/artifacts/pcsx2/ref/bf1_kas2/pairs/cp4_cp11_dock_two_spawn.png`, box (540,110,630,150): PS2 (126,114,85) vs port (46,45,27); the port also plants grass tufts there. RE (§5): the terrain's texture layers and blend weights at that spot (`analysis/re-terrain-heights.md` — the layer bitmask and the `ALPH` stream; `analysis/a2-bake-formats.md`; HANDOFF HOLD 10(d)), which layer the PS2 shows and why the port shows grass; then a census of the same rule over the other maps' terrain. GATE: the cause named with its bytes (§5 header, [O], verifier) and a follow-on [REBAKE]/[GOLDENS] build entry. Files: `analysis/re-terrain-heights.md` (or a new `analysis/re-terrain-layers.md`), `analysis/RE_LOG.md`, scratch `engine/artifacts/lead_tmp/re/ktex/`.

## Merged into TALPH (planner 2026-09-26)
Same cause as TALPH: the port bakes only LTEX[0]. BF1 KAS2's slope is layer 1 `kas2_main_2` (mean 124,110,88, pale sand,
painted in 60 patches); BF1 KAS1's base LTEX[0] is the sand `kas1_main_2` with the green layers on top. TALPH's
acceptance carries this card's Dock Two box and the KAS1 poses.
