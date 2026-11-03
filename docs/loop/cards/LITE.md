---
id: LITE
title: The port runs brighter than the PS2 on static scenery: BF2 Mygeeto ~60 % brighter at mid-distance, BF1 Theed's shaded columns bright beige where the PS2's are d
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why / Evidence / Acceptance (rewritten 2026-09-25, lane 1 — the RE is DONE)
- **Why.** The port rendered BF2's static scenery 1.3-2.6x brighter than the PS2, and Theed's shaded columns beige
  (F15, F20, F27).
- **Answer, `analysis/re-model-lighting.md` `[O]`** (29 SUPPORTED / 2 PARTLY, folded in).
  - The port drops the disc's per-vertex COL0. The PS2 multiplies it into the lighting, or adds it on 0x200 segments.
  - The port ignores "prelit" models (no ambient, no `Static()` lights), BF2's Light2, BF2's terrain rule (lightmap only)
    and BF2's far-scene-only fog.
  - It uses BF2's sun colour raw as linear.
  - BF2's 0.5 scale is `Effect("HDR")`, which is neutral.
  - F15 is the sun direction (SUND).
- **Evidence.** PS2 patched save states plus the port with the PS2 rules at the same poses (§7, §8):
  - MYG1 floor 52 -> 31 (PS2 28);
  - TAN1 frame 137 -> 109 (PS2 103);
  - COR1 floor 92 -> 43 (PS2 46);
  - HOT1 snow 242 -> 229 (PS2 214);
  - NAB2 columns 97 -> 59 (PS2 57).
- **Split into** LITEB (models, build, 5), TERL (terrain, RE, 5), FOGN (BF2 fog, build, 3), and notes on SUND, GLOW,
  SUNG and BACKL. Report: `engine/artifacts/loop/reports/20260925_1815_re_LITE.md`.

## Original QUEUE.md entry (verbatim, 2026-09-25)
LITE. [TODO — **NOTE 2026-09-25 (WCOL's RE, `analysis/re-water-colour.md` §1.10/§2/§4): (a) the beds under BF1 NAB2's canal and BF2 FEL1's lake are 2–2.5× BRIGHTER in the port than on the PS2 (patched save states); (b) the port's sun direction is not the PS2's on any map — queue SUND; (c) [H] lead for the BF2 half: before its water draws BF2 sends a global colour scale `DAT_007c5754` (0.5 on six maps, 1.0 on BF2 YAV1 and DAG1; set by `FUN_0057fd40` from `FUN_00456098`) through VU colour command 0x43a into the light-colour slots VU[11]/[20]/[29]/[38] — if lit BF2 draws read those, the PS2 lights most BF2 maps at half the authored light colour.** **NOTE 2026-09-25 (PXC batch 3, FINDINGS F27): a BF2 effect — a fixed lower-right-box census puts the port/PS2 luminance ratio at median 1.30 over 17 BF2 maps (14 above 1.1; FEL1 2.58, MYG1 1.91, TAN1 1.79, END1 1.66, COR1 1.56) and 0.93 over 15 BF1 maps (both directions) [D, crude]. Clearest: BF2 COR1 floor (34,37,42) vs (82,79,82); both Hoths' snow clips in the port (BF2 `cp1_cp6_postA_v2` 2.1 % vs 51.6 % of pixels ≥ 245; BF1 `cp2_cp6_postA_v1` 0.0 % vs 56.1 %); BF2 TAN1 halls. So look first for a BF2-only light term (BF2's lighting / `analysis/re-fog.md` BF2 half) — not a shared one.** ADDED 2026-09-25 (lane 1) by PXC batch 2 (FINDINGS F15, F20); **[RE] FIRST**; FREE; overlaps SUNG (fog) — read SUNG's answer first if it has run] **The port runs brighter than the PS2 on static scenery: BF2 Mygeeto ~60 % brighter at mid-distance, BF1 Theed's shaded columns bright beige where the PS2's are dark purple-grey** [RE] — evidence `engine/artifacts/pcsx2/ref/bf2_myg1/pairs/cp2_cp1_con_post1_{spawn,v2}.png` (band (0,60,640,200): PS2 (59,62,62) vs port (97,98,98); (48,51,50) vs (78,80,80)), `…/bf1_nab2/pairs/cp4_cp4_rotunda_{v3,v4_up,v5_up}.png`. RE (§5): which term — the fog colour or density (`analysis/re-fog.md`), the static models' vertex light / baked colour (`analysis/re-sky-lighting.md`), the ambient — take the port's draw apart at one pose (a scratch `pxc_pose.rs` variant, one term per render, patch means against the PS2 frame). GATE: the term named with a before/after patch mean at that pose (§5, [O], verifier) + a build entry. Files: `analysis/re-sky-lighting.md` (a dated §) or `analysis/re-fog.md`, `analysis/RE_LOG.md`, scratch `engine/artifacts/lead_tmp/re/lite/`.

## Notes (planner 2026-09-25)
- Folds in HOLD 11(a) (archived HANDOFF): ~10 % of both Hoth goldens clip to pure white — is the PS2's terrain light a SUM
  of ambient and sun, a modulate, or a fixed 2x over `LUMI`? "Do not rescale the ambient without an answer." F27's Hoth
  snow-clip numbers are the same symptom. Related: BACKL (the unrendered BF1 back light), SUND (sun direction).
