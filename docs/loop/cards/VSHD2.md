---
id: VSHD2
title: Vehicle shadows drawn (BF1): stencil the sun-extruded shdw volumes of every BF1 vehicle after the opaque passes and multiply the stencilled pixels by 0.625 (black) - the PS2's look at the X-wing pose; never on BF2
status: archived
issue: 66
---

Historical technical record. Recorded state before archival was `todo`. Current scope and ownership are on [issue #66](https://github.com/sprucey149/openbf/issues/66).

## Why
BF1 darkens the ground (and anything opaque) inside every vehicle's sun-extruded shadow volume, x 0.625, black; the port
draws no shadow at all. Measured on the PS2 (BF1 NAB1, the parked X-wing at VREST's pose): a band of ground in front of
the X-wing, 3.5-5.2 % of the frame, displayed at 0.585 of its unshadowed brightness (`analysis/re-vehicle-shadow.md` 5;
`engine/artifacts/lead_tmp/vshd/ps2/xw_texa_sheet.png`: TA0 0x80 paints the exact region black). Showcase maps: BF1 NAB1
(AAT, STAP, fighter tank; AT-ST, bike, X-wing), YAV2 (spider walker, AT-ST, fighter tank). BF2 draws none (re 6): never
on a BF2 map.

## What to build (re-vehicle-shadow 2-5, 7)
- Per BF1 vehicle with baked pieces (card VSHD), per frame: L_world = the map's `sun_dir` (= PS2 DAT_0069c590).
  Length = factor x the terrain hit fraction of a segment from a along L_world x factor: flyer 500, hover / walker 150;
  a = collision-sphere centre + (0, r, 0) (walker: pos - L d raised by r - d, d = leg radius / (2|L.y|) when L.y < -0.5,
  else the radius) - ELF constants; the port's terrain ray stands in for FUN_00474680 (terrain only, 1.0 on a miss).
- Per piece in its node's frame (`dummyroot`, gear, hatches; walker legs through `walker_pose`): faces lit when
  n . L_local < 0 (n from the first non-degenerate cross product of the face's consecutive vertices); silhouette = twin
  pairs whose faces differ; side quads extruded by L_local x length; caps = the faces with n . L_local > 0 and their copy
  moved along L.
- Stencil after the opaque world + models (PS2 buckets 3 and 5), before water, the far scene and translucents (8, 9-10,
  0xd); count with wrap; then one full-screen pass: stencil != 0 -> colour x 0.625 in the PS2's frame units (the port's
  `Rgba8UnormSrgb` blend is linear - decide the conversion and say it in the module header).
- Named deviation (FEEL/DEVIATION note): z-fail with caps at every distance; the PS2 uses z-pass (no caps) past 30 m +
  the model radius, which is wrong when the eye is inside a long volume.
- The depth target gains a stencil aspect (`Depth32FloatStencil8` needs a device feature; `Depth24PlusStencil8` changes
  depth precision): choose, measure the golden moves and declare them.

## Acceptance
- At VREST's NAB1 X-wing pose (`game --map <bake>/nab1 --headless --mission missions/nab1_con.ron --walk-replay
  lead_tmp/vrest/build/port/idle.ron --cam <pose of lead_tmp/vrest/build/ps2/nab1_xwing_pose.json> --capture-frame
  --size 640x480`): render once with the factor forced to 0 (black, like the PS2's TA0 0x80 frame) and compare the black
  region with the PS2's `lead_tmp/vshd/ps2/xw_g1_ta80_gate1.png` black region outside the PS2 soldier and HUD: IoU >= 0.6
  (print it); and the shipped factor's luma ratio over the port's own region (shadow on / off renders) 0.55-0.66
  (PS2: 0.625 in the frame buffer, 0.585 displayed).
- A render test: a box's volume over a plane under a known sun darkens exactly its analytic shadow (pixel count within
  2 %), unchanged with the eye inside the volume (the z-fail case), and nothing on a BF2 map.
