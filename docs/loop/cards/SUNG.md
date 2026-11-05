---
id: SUNG
title: The PS2's sun glare and horizon haze: a white bloom toward the sun (BF1 KAS2, BF2 KAS2) and BF2 KAS2's near-white horizon over the sea, where the port has neith
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why / Evidence / Acceptance
See the original entry below; the first session to work this card rewrites this section into short Why / Evidence / Acceptance lines and fills `files:` and `declares:`.

## Original QUEUE.md entry (verbatim, 2026-09-25)
SUNG. [TODO — **NOTE 2026-09-25 (WCOL's RE, `analysis/re-water-colour.md` §4): the PS2's glare in `bf1_kas2/cp5_cp10_dock_one_v3_up` sits where the PS2's own sun vector puts the sun; the port's baked sun is 90° away on BF1 (z-mirrored on BF2) — queue SUND first, then place any glare by the corrected vector.** **NOTE 2026-09-25 (PXC batch 3, FINDINGS F30): two more glare frames — BF1 HOT1 `cp2_cp6_postA_v3_up` (0,0,200,100) PS2 (176,189,194), 10.5 % ≥ 235 vs port (129,158,194), 0 %; BF2 TAT2 `cp1_cp2_postA_v3_up`. LOW-MEDIUM under COMPARISON_RULES (an SD bloom is not a defect).** **NOTE 2026-09-25 (the maintainer's `engine/artifacts/pcsx2/ref/COMPARISON_RULES.md`): a bloom or glare that is a screen-space effect tuned for 480 lines is NOT a defect to copy — the RE must say whether F3 / F7 are authored (sky / sun config, fog) or a post effect, and only the authored part becomes a build. The haze itself survives PCSX2's software renderer (FINDINGS "Batch 2").** ADDED 2026-09-25 (lane 1) by PXC batch 1 (FINDINGS F3, F4, F7); **[RE] FIRST**; FREE] **The PS2's sun glare and horizon haze: a white bloom toward the sun (BF1 KAS2, BF2 KAS2) and BF2 KAS2's near-white horizon over the sea, where the port has neither** [RE] — evidence `engine/artifacts/pcsx2/ref/bf1_kas2/pairs/cp5_cp10_dock_one_v3_up.png`, `engine/artifacts/pcsx2/ref/bf2_kas2/pairs/cp0_cp4con_post3_{spawn,v1,v4_up}.png` (the horizon box (250,100,480,140): PS2 (234,234,231) vs port (183,180,171)); F4, BF1 KAS2's hazier far mountains (`…/cp5_cp10_dock_one_v2_up.png`), rides along. RE (§5): what draws it — the sky config's sun / flare / glow entries (`analysis/re-sky-lighting.md` has `SunInfo`; is anything flare-like parsed and never drawn? — an absence claim needs the search), a post pass, fog on the sky dome and the sea (`analysis/re-fog.md`), or a brighter dome texture. GATE: `analysis/re-sky-sun.md` (§5 header, [O], verifier) with each of F3 / F4 / F7 attributed or left [H]; follow-on build entries. Files: `analysis/re-sky-sun.md`, `analysis/RE_LOG.md`, scratch `engine/artifacts/lead_tmp/re/sung/`.

## Notes
- 2026-09-25 (LITE RE, `analysis/re-model-lighting.md` §1): BF2's white glare is the per-map `Effect("HDR")` post pass (FUN_004665d0 → FUN_0058b188 threshold at GlowThreshold x 255, FUN_0058b9a8 blur NumBloomPasses, added in the composite). It is screen-space, so COMPARISON_RULES applies, and whether to draw a modern bloom from it is the maintainer's call. BF1 has no such effect in this RE's reading (unchecked).
- 2026-09-26 (SUND RE, `analysis/re-sun-direction.md` §5, [O]):
  - **Answers "is anything flare-like parsed and never drawn": yes, BF2's `SunFlare` block.** B2 FUN_0046e998 reads it
    (vtable 0x63acc4; registered for hash 0xbdf357b3 "SunFlare"). It holds `Angle`, `Color`, `Size`,
    `HaloInnerRing` / `HaloMiddleRing` / `HaloOutterRing` (sic), `SpikeColor` and `SpikeSize`, and stores the flare's
    own direction at +0xa0 from its Angle.
  - Which chunk carries SunFlare, and whether mungefs parses it: unread.
  - BF2 ignores `SunInfo` entirely (its reader FUN_003be738 drops the Color and has a dead Angle case). BF2's sun sprite
    / flare direction is therefore the SunFlare Angle, NOT the lght sun.
  - BF1 places anything sun-relative by 0x69c590 (FUN_0049b8a8 hands it to about 20 callers).
  - After SUND lands, the port's `sun_dir` IS that vector.

## Note (PXWA, 2026-09-29, lane 2): BF1 NAB1's sky is brighter on the PS2 even away from the sun
The PCSX2 walkabout at NAB1 cp2: sky boxes PS2 vs port — b090 (0,40,640,120) (159,176,193) vs (140,150,161); b180
(147,168,200) vs (130,143,166); b270 (173,180,192) vs (141,149,164) (plus the glare toward the sun); the clear blue at
b000 (99,138,190) vs (98,123,160) — `engine/artifacts/pcsx2/ref/bf1_nab1/walk/pairs/cp1_cp2__sheet.png`. The KAS2, YAV1
and BES1 skies match at the walkabout's level pitch, so this is NAB1's dome/clouds, not a global offset. MEDIUM.
- 2026-09-29 (PXWB part 3, FINDINGS F59): BF2 KAS2's sea glare at the walkabout stands — `pcsx2/ref/bf2_kas2/walk/pairs/cp2_cp3con__sheet.png` b090 near sea (0,280,200,340) PS2 (89,93,87) vs port (53,51,44); `cp1_cp1con__sheet.png` b090 horizon (260,170,640,230) (205,204,201) vs (148,147,141); HOT1's big white sun bloom at every open post's b270 (`bf2_hot1/walk/pairs/cp0_cp5__sheet.png`).
- 2026-09-29 (PXWB part 4, FINDINGS F64): BF2 POL1 toward the sun — a rainbow lens-flare ring and a white glare on the PS2, a small white sun point in the port (`pcsx2/ref/bf2_pol1/walk/pairs/cp1_cp6con__sheet.png` b090: sky (0,0,640,140) PS2 (96,91,101) vs port (38,27,31); the sun box (380,80,480,160) (143,145,168) vs (74,60,61)); the lit mist under it is DUSTE's.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 3 (kept).

`acceptance: needed` (planner 2026-09-30): no measurable acceptance yet (a parity delta, a leak-census count, a sim replay hash or a pixel test). Write one before the card is picked.

## Note (2026-09-30, GLOW RE, `analysis/re-glow.md` §1.3 / §2)
BF2's glare is the `Effect("HDR")` bloom (threshold on luma ≥ 2·GlowThreshold of display white, per-map passes/weight,
σ ≈ 7.8 % of the screen height at 5 passes) — card GLOW builds it. Three BF2 sky domes (GEO1, KAS2, MUS1) carry MTRL 0x10
with an alpha glow mask on 2–15 % of their texels: those texels draw ×(1 + As/128) (up to ×3) and bloom — the likely
source of BF2 KAS2's sun glare. BF1 has no HDR; its brighter skies are the DomeInfo Filter (card BFLT).

## Local Geonosis follow-up (2026-11-02)
BF2 sky material 0x10/0x14 handling and sky bloom gathering now exist on `map-atmosphere`; `geo_sky` retains its source glow mask (0..115) instead of alpha 255. This restores the bright authored sky glare in the Geonosis reference. The solution also consumes KAS2/MUS1 masks after rebaking. See `analysis/re-bf2-geonosis-atmosphere.md` for source attribution, captures, occlusion tests and remaining limits. This is existing implementation work: do not duplicate it. SUNG remains open for its other maps and original-game acceptance.

### Hoth maintainer trial (2026-11-02)

The BF2 root SunFlare scope now has a parser, optional manifest field and native
the live PS2 object; source occlusion and scene clear have GPU coverage. The
follow-up replaces analytic rays and arbitrary circles with the original mask
expansion, spike fans, thirteen polygon reflections and alignment glare, and
feeds the existing HDR bloom. Exact partial-occlusion source-mask filtering
remains unverified. See `analysis/re-hoth-atmosphere.md`.
This existing implementation does not complete SUNG's remaining maps or acceptance.

## Source reconciliation — November 4, 2026

The #118 atmosphere source and the BF1 shared-runtime playtest trial are now
integrated at `4cda5ab4`. Earlier scope/hold statements above describe their dated
checkpoint. Current implementation boundaries and still-open native evidence
are in `analysis/re-bf2-geonosis-atmosphere.md` and
`analysis/re-hoth-atmosphere.md`; this note does not close original-game or
all-map acceptance.
