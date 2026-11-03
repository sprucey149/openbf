---
id: ADDB
title: Additive draws blend in linear light where the PS2 adds bytes: BF1's half-bright hologram column reads about 60 % of the PS2's on a dark wall (Kashyyyk Docks cp3 +60 levels against +82, Endor cp2 +46 against +72), BF2's reads 115 % in red and 75 % in green and blue (Death Star cp3 +99 / +36 / +29 against +86 / +42 / +40) - the bolts' glow, the muzzle flash and the additive particles share the pipeline
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Found by card CPHOL (2026-10-13). `render/src/bolt.wgsl` `fs_bolt_glow` turns the vertex colour to linear light and the (One, One) blend adds it to a linear target; the GS adds Cs x As to the frame's bytes. On a dark background the two differ by the curve: a half-bright source adds far less, a full one more in its strongest channel.

## Evidence
- `analysis/re-point-lights.md` 15.2 and 15.7; boxes by `engine/artifacts/lead_tmp/cphol/tools/boxpair.py` on the stored frames bf1_kas2 cp3_cp3_cityhall_v2 (545,110,595,200), bf1_end1 cp1_cp2_postA_v1 (500,110,580,180), bf2_dea1 walk/cp3_cp3_b000 (290,80,380,200); the port's renders in `lead_tmp/cphol/par_holo3/render`.

## Build / measure
1. Measure first: the same three boxes plus one bolt glow and one muzzle flash pair against PS2 frames, port against PS2, per channel.
2. Options to weigh: an additive pass into a display-encoded target, or a source term fitted per draw; say what each moves (every golden with a bolt, flash or effect in view).

## Acceptance (run and paste)
- The three boxes above within 10 levels per channel of the PS2's added value; `scripts/parity.py`: no map up by more than 0.5; the goldens that move are declared.

- 2026-10-22 (card LFLR build): one more case for this card. UTA1 cp2_con_cp1a b180, box (0,0,200,330) round the lamp head: PS2 91.9/78.3/76.4, main 70.0/64.7/66.8, LFLR branch (lamp flare + cone, additive) 81.5/71.2/69.6 - the flare and cone add about half what the PS2 adds (`lead_tmp/lflr/sheet.py`, `lflr_sheet.png`).
