---
id: B1LL
title: BF1's local lights: on the PS2 every BF1 model draw takes up to 4 omnis - the command posts' hologram light (r 8 at the hp_hologram bone; friend green, enemy red, neutral grey), EntityLights at their bones on 8 maps (HOT1 43, KAS1 49), the point weight to the 4th power - and the port lights BF1 with none
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
A friendly command post on the PS2's BF1 washes the deck round it green, an enemy one red, and lights the soldiers
standing at it; Hoth's corridors and Kashyyyk's huts carry lamps that light their walls. The port lights BF1 with the
sun and the ambient only (`point_light::still_lights`: "A BF1 map has neither"; the bake attaches EntityLights for
BF2 alone). From card B1DK's RE, `analysis/re-kamino-decks.md` §6 (helper report `lead_tmp/b1dk/re/AGENT_BF1.md`).

## Evidence
- PS2 knock-out (`lead_tmp/b1dk/ps2/frames/k1__ctl2.png` against `k1__noomni.png`, BF1 KAM1): with no omni filed
  for the view the green wash round the friendly post is gone - box (200,215,420,235) 93.6/135.5/111.6 -> 62.2/71.4/79.7.
- Code: `FUN_00496248` gives every RedModel draw (static, skinned, prelit) up to 4 omnis by BF2's score
  (`FUN_00493cf0`); the terrain takes none; VU1 0x21f-0x285: the point weight is ((1 + N.L)/2)^4 x (1 - min(1,
  d^2/r^2))^2 - the 4th power where BF2's is the 2nd.
- The post's light: the hologram's omni, range `HoloLightRadius` 8, at the bone `hp_hologram` of the post building's
  model ((0, 0.320, 0) in `com_bldg_controlzone`, (0, 1.363, 0) in `com_bldg_controlzone_large`, END1's bunker panel
  and HOT1's shield generator elsewhere - `re/hp_census.py`); colour = the viewer's relation colour x the hologram's
  intensity: friend (100,200,100), enemy (200,100,100), neutral (192,192,192), a side-5 locals team (223,223,32).
  RAM: 1856 of 1856 lights over 150 states (`re/holo_rule.py`).
- EntityLights: BF1 keeps an attachment only when the class model's skeleton HAS the bone (the port's present skip
  rule). RAM (`re/omni_census.py`): HOT1 43, KAS1 49, RHN2 7, BES2 4, END1 4, KAS2 4, GEO1 1; TAT1 5 by the disc.

## Build
1. `game/src/point_light.rs`: BF1 posts in `still_lights` and the battle's `submit` (`SceneGpu::post_lights`), BF1's
   colour table, the light at the post class's `hp_hologram` bone (bake the bone's offset per post class; it also
   corrects BF2's large posts, 1.36 m where the port has 0.32).
2. `bake/src/map.rs` / `entity_light.rs`: attach EntityLights on BF1 maps too, by the bone rule (no origin fallback:
   that is BF2's, card B1DK). `OmniRadius` defaults to 10; a 4-value `Color` is rgb x a/255.
3. `render`: the point weight's power per game (4 on a BF1 map).
4. Not here: the hologram's 2 s fades and flicker dips (PROVISIONAL steady, as BF2's), mines' and explosions' lights
   (the doc's §6 "Other makers" - a note for FXP5).

## Acceptance (run and paste)
- A light-list referee: for every BF1 map with a walk state, `PXC_LIGHTS=1 pose_render` lists the view-0 hologram
  lights and EntityLights that `lead_tmp/b1dk/lights1.py <state>` prints (place within 5 cm, range, colour for the
  state's owners) - HOT1's 43 and KAS1's 49 included.
- `ref/bf1_kam1/cp1_cp10_post2_v1` (a friendly post in view; PS2 frame washed green): the port's box round the post
  gains green over main, and the pair's block error falls (main 29.4; LSUM alone 17.4; the s^2 prototype 11.6).
- `walk/cp3_cp12` (the cloning room's large post) x 4 bearings: mean block error not above main's by more than 0.5
  (the s^2 / 0.32 m prototype was +1.6: the 4th power and the 1.36 m bone are the fix to confirm).
- `scripts/parity.py`: no BF1 map up by more than 0.5; bf1_kam1 not above its value after B1DK.
- Tests: the term's power on a BF1 map (a vertex edge-on to a light takes 1/16 of its colour, 1/4 on BF2); a post's
  relation colour; the bone height.

## Note from card B1DK's landing (2026-10-05, 71c3855)
B1DK is on main. BF1 Kamino's deck box (450,330,640,470) is now within 14 levels of the PS2's at six of the eight walk
poses; blue at bearing 270 is 14.2 (cp0_cp2) and 14.3 (cp4_cp4) short (PS2 84.6/101.6/115.8, port 74.6/88.1/101.6).
Re-take that table in this card's acceptance (`reports/20261005_1535_card_B1DK.md`; `lead_tmp/b1dk/boxes.py`).
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.

## Note (planner 2026-10-12 23:40)
- Card CPHOL (5, new) draws the post's hologram (beam + icon) in both games; this card stays the BF1 omni LIGHT. Not
  folded: a different renderer path and a rebake. They share the `hp_hologram` bone height - whichever builds first
  bakes it, the other reads it.

## Build (2026-10-22 05:30, lane 1, branch b1ll 4b12c8d - parked for the RTX)
- Built: BF1 posts' hologram light (still + battle) at `holo::bone_height`, BF1 colours incl. locals (223,223,32) in stills;
  BF1 EntityLights baked by the bone rule with BF1's setter (4-value Color = rgb x a / 255 integer, no Static); BF1
  packing (b + 1) >> 1 (KAS1 redlight 253 -> 127, the port's BF2 rule gave 126); the s^4 facing weight on BF1 maps.
- Referee `lead_tmp/b1ll/referee.py` (out: referee.txt): 1203 view-0 lights over the BF1 walk states, 1193 within 5 cm,
  range equal; colour 1141 exact, 11 hologram dips, 41 owner changed since the mission start (walker's captures);
  10 = HOT1 'shield' + END1 'cp1', uncapturable posts the layout does not load -> card NOCAP.
- Parity (Intel adapter both sides, scratch bake `lead_tmp/b1ll/bake`): all 954 pairs 9.517 -> 9.229; bf1_kam1
  13.103 -> 12.853, geo1 -2.60, kas1 -1.72, rhn2 -1.70, tat1 -2.08, hot1 +0.09 (worst), BF2 0.000.
  cp1_cp10_post2_v1 14.51 -> 9.52; walk/cp3_cp12 x4 mean 18.40 -> 18.94 (+0.54, bar 0.5: main's floor there is
  already ~10 levels over the PS2 at three bearings, the enemy light adds R+6 G+3 B+3; a droideka covers the PS2 frame).
