---
id: HNEAR
title: A hover rides to the hub nearest its post when its graph ends short (ai::HOVER_NEAREST, built by VDROP and off): BF2 POL1's riders get out over no floor and tat1's speeder bikes crash
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Card VDROP built the rule for hovers and left it off. With it on (8 seeds, `lead_tmp/vdrop/sw_near` against
`sw_law3`): rides 3753 -> 5366 (BF2 POL1's combat speeder 0 -> 304, BF1 NAB2 188 -> 348), crashes a ride unchanged
(0.06 -> 0.05), rounds finished 255 -> 256 of 256, recycled 230 -> 190. Against it: tat1's `veh` deaths 16 -> 61
(5.6 a round; its bikes crash 28 times in 450 rides against 8 in 297) and BF2 POL1 gets 22 floor deaths in 8 rounds
(none before): riders leaving the speeder at a drop hub with no floor under the exit point.

## Evidence
`lead_tmp/vdrop/sw_near/seeds.md`, `sw_law3/seeds.md`; `python3 lead_tmp/vdrop/hov.py <dir> [map]`;
`lead_tmp/wai/build/cmp.py <a>/seeds.md <b>/seeds.md`. Neither Hoth has a hover class: nothing to gain there.
