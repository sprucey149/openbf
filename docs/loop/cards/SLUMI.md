---
id: SLUMI
title: Soldiers in light-map shadow: on maps with DomeInfo TerrainColorDarkening the PS2 draws a soldier at min(1.2 LUMI + 0.3, 1) of his colour (BF1 NAB1, TAT1, END1, TAT2; BF2 END1, KAS2) - the port draws every soldier at 1.0
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
On the PS2 a soldier standing in the terrain's light-map shadow is drawn darker: the soldier draw's colour is
min(1.2 x LUMI + 0.3, 1) at his feet (LUMI = the terrain light map, bilinear, /255), on maps whose sky `DomeInfo` carries
the valueless key `TerrainColorDarkening` (0xd8a93a22). The port draws every soldier at colour 1.0. Maps: BF1 END1,
NAB1 (+_H), TAT1 (+H), TAT2 - showcase NAB1 and TAT1 - and BF2 END1, KAS2 (BF2 also needs its terrain flag DAT_0060bed4).
A soldier on LUMI under 0.583 is drawn at down to 0.3 of his colour, so in tree / building shadow the port's soldiers
are too bright on those maps.

## Evidence
- `analysis/re-soldier-light.md` §4 [O] (card SBLT): B1 `FUN_002013a0` 113440-113447 / 113567-113577; flag
  `DAT_005229bc` set only by the DomeInfo loader `FUN_002e29e0` 270363; B2 twin `FUN_00239bb0` 153625-153631 with
  `DAT_0060bed4` && `DAT_0060bedc` (`FUN_003bcd50` 416290, from 0xd8a93a22 or 0xed4a20e6).
- RAM (`lead_tmp/sblt/ramcensus.txt`): BF1 flag 1 on END1, NAB1, NAB1h, TAT2; BF2 both flags on END1, KAS2 only.
- Disc (`lead_tmp/sblt/disckeys.txt`): the key in END1, NAB1(+_H), TAT1(+H), TAT2.
- The LUMI lookup the port already models for foliage: `bake::foliage::Lumi` (B1 `FUN_00473bc0` / B2 `FUN_0055f068`).

## Acceptance
- The bake carries the flag (sky `DomeInfo` key 0xd8a93a22; check where `mungefs::sky` files a valueless DomeInfo key)
  and the runtime has the terrain LUMI; `soldier_vis` sets `SkinnedDrawCmd::color` = (t, t, t) with
  t = min(1.2 L + 0.3, 1) at the soldier's (x, z) on flagged maps, 1.0 elsewhere. A unit test of t against a hand
  bilinear sample (two points: an open cell t = 1, a shadowed cell t < 1), mutation-checked.
- A PCSX2 pair: a BF1 NAB1 soldier in a shadowed LUMI cell (a save state; or patch `DAT_005229bc` to 0 in RAM for the
  control) against the port at the same pose - the soldier's mean luma within 3 levels of the PS2's ratio
  (flag on / flag off).
- BF2: decide `DAT_0060bed4` (read in 20+ places; [H] "terrain loaded") before applying the BF2 rule.
