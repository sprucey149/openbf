---
id: SPWS
title: A battle's first tick starts 39 sounds at once: every soldier's weapon-equip sound and first step (BF2 Kamino, 16 a side, windowed: sum of gains 13.6, nearest 2 m; the offline mix peaks 1.59 x full scale) - measure what the PS2 starts at a spawn and gate the port's
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Found by card SNDB's first real snapshot (2026-10-07): the black box's only OVERLOAD line in 59 s of BF2 Kamino was tick 1.
Every unit spawns on the same tick, so every unit's equip sound and first footstep start on it. The maintainer hears it as a burst when a battle begins.

## Evidence
- `engine/artifacts/lead_tmp/sndb/win/bb/prev1.json` (build b870afc +dirty = branch sndb), digest `lead_tmp/sndb/win/digest.txt`:
  `OVERLOAD -58.97s tick 1: 39 starts, sum 13.58: com_weap_inf_equip_med x11 (4.10), com_weap_inf_equip_sm x13 (4.92), com_weap_inf_equip_lg x6 (1.80), rep_inf_trooper_walk_metal_left x9 (2.76) - nearest 2 m`
- `python3 scripts/tools/mix.py <that file> engine/assets_baked`: peak 1.59 x full scale, 84 frames over it, all in second 0 (`lead_tmp/sndb/win/prev1_mix.wav`).
- Unknown: whether the PS2 plays an equip sound on a spawn at all (it may only on a weapon SWITCH), and whether a soldier that has not moved steps.
