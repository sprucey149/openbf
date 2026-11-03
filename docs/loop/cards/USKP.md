---
id: USKP
title: Class skins seen in a battle: a picture pair of the port's CIS droid skins (red / orange / camo / blue) and the sharpshooter / desert rebels against the PS2's frames - no golden shows a skinned class today (USKN landed the swap with census + unit tests only)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
USKN (eb6ff83) draws a class's OverrideTexture / OverrideTexture2 on its model's MNAM group, proved by a disc census (32 classes, every texture baked) and unit tests. Its Acceptance 3 also asked for a picture pair against `lead_tmp/unit-audit/pcsx2_bf2/cis_droid_skins.png` and the BF1 class-select sheets; no golden draws a skinned class (the unit gallery draws each GLB's own textures), so the pair was not made.

## Acceptance
- A port frame of the BF2 CIS assault / engineer / assassin droids and BF1's sharpshooter in a battle (pose_render with the class's skins, or a walkabout still), beside the PS2 frames; each class's sheet is the PS2's.
- Fold in, if cheap: the unit gallery draws one skinned class per model so a golden guards the swap.
- Known gaps left by USKN (note, not this card): a class whose body falls back to its team's first class takes its skins over the fallback's groups; BF2's 10/s limit under a scale below 1 and the scale's reset bracket (soldier +0xc14) are not built; the droideka's bone list is not built (no droideka soldier is fielded).
