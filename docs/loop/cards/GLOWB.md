---
id: GLOWB
title: BF2's HDR bloom: the halo around lamps, strips and lava on the 13 HDR maps (the glow's second half; GLOW landed the self-lit segments)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why / Evidence / Acceptance
- **Why:** on the PS2 every BF2 lamp, strip, skylight and lava sea on the 13 HDR maps carries a white halo; GLOW made the surfaces glow, the halo is still missing (MYG1's corridor strips, POL1's skylight, COR1's light columns, UTA1's hangar lamps).
- **Evidence:** `analysis/re-glow.md` §2 [O]: luma601 of the half-scale frame >= GlowThreshold·255 (i.e. >= 2·GlowThreshold of display white BEFORE the clamp) → 4× down → NumBloomPasses×2 five-tap passes (tap weight int(int(MaxTotalWeight·127)·0.2)/128, taps at (±1,±1)·n/2 texels) → magnified and ADDED; σ ≈ 7.8 % of the screen height at 5 passes. Measured on MYG1 (§2.5): +16 frame luma, the cascade reproduces the PS2's bloom within ~10 %. Per-map numbers are baked: `manifest.hdr` (ManifestHdr, GLOW).
- **The hard part:** the threshold and the halo's strength read the PRE-clamp brightness (up to 2.0 of display white): the glow segments' ×2.98 and overbright lit surfaces. The port's scene target is 8-bit sRGB and clamps, so the bloom needs the pre-clamp value — a float scene target (Rgba16Float, then a resolve pass to the surface) or a second colour attachment carrying min(2, pre-clamp)/2. Every world pipeline is keyed on the target format; the terrain-light and DomeInfo filter passes read frame copies.
- **Acceptance:** MYG1 cp4_cp5 median block error down from GLOW's landing value (card PARITY); the named boxes UTA1 cp3_con_cp1 b090 (0,0,120,80) toward (130,132,138), POL1 cp0_cp2con b000 (260,0,480,50) toward (252,252,251); no BF1 golden moves; no other map's parity mean up > 0.5. Native-resolution blur (COMPARISON_RULES), radius scaled with the target height; split screen keeps it (the PS2 drops HDR there — reversible, FEEL_NOTES).
## Note (KAMR RE, 2026-09-30): Kamino's deck highlight
BF2 KAM1's decks were 1.5-2.1 x dark at 0994d9a and match the PS2 since BPNT landed (5c73167: (72,76,81) vs (72,78,86)
at cp2_cp3 b000). What is left is the highlight at bearing 270: PS2 (157,163,171) / (143,151,162) / (137,142,150) vs the
port's (108,112,117) / (115,120,125) / (117,121,126) at cp2_cp3 / cp5_cp5 / cp4_cp4 (box (430,340)-(640,470)) — bloom
on the white deck lines is the candidate. `analysis/re-kamino.md` §5.

## Note (card B1DK's RE, 2026-10-05): Kamino's bearing-270 highlight is the pass plus the bloom it triggers
PS2 knock-outs at BF2 KAM1 turned to bearing 270 (`lead_tmp/b1dk/ps2/frames/k2w__*.png`, box (430,340,640,470)):
control 123.8/119.0/127.0, bloom off 105.7/102.4/109.0, specular pass off 95.0/90.8/98.4, both off 95.7/91.6/99.2 -
the pass adds about 10 and the bloom about 18, and the bloom adds nothing once the pass is off (the highlight takes
the white deck lines over the threshold). At the stored pose `walk/cp2_cp3_b270` the same box reads PS2 157/163/171
against 107/114/122 with LSUM + B1DK prototyped (main 76/79/84). `analysis/re-kamino-decks.md` §1.3, §8. A referee box for this card.
