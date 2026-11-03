---
id: AIMBX
title: The acquisition box per soldier class: the port uses one RAM-measured box (r 1.416) for every unit; the PS2 reads each model's box at vtbl+0x38()+0x1c (seven shapes, r 1.387-1.704) — find which chunk it is (not the modl vis box: y max 2.0877 vs RAM 2.099) and carry it per class
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
AIMF (73632e1) landed the PS2 acquisition with `unit::AIM_BOX_MIN/MAX`, one BF1 soldier box read from RAM (`analysis/re-aim-assist.md` §3.6), PROVISIONAL for every class. Bigger classes (r up to 1.704) are easier to magnet on the PS2.

## Evidence
re-aim-assist §3.6 [V-RAM]: seven box shapes r 1.387-1.704; the modl vis box (`mungefs::model::Model::vis_box`, stormtrooper max (0.929476, 2.08769, 0.215175)) does not equal the RAM box (0.929, 2.099, 0.204). Tools: `lead_tmp/aimf/cone_ram.py`, the 23 BF1 states.

## Do / Acceptance
Identify the box (RAM *(O+0x240) +0x80/+0x90 vs the modl chunks: vertex box, vis box, a collision prim, a skinned bound); carry it per soldier class (UnitClass -> UnitSpec -> Unit::aim_bounds); a test that two classes get their own r. Declares: none expected unless a referee has a player aiming.

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 2 (kept).
