---
id: CPHOL
title: The command post's hologram (the maintainer's INBOX 2026-10-12 19:05, split out of LFLR): over every post a translucent additive beam in the VIEWER's relation colour - his team's posts blue (BF2 own 1,86,213), the enemy's red (200,100,100), a neutral post grey (192,192,192) - and the owner's spinning translucent team icon (com_icon_*) at 3.5 m, none on a neutral post, a 2 s fade on a capture, the flicker dips; the port draws none of it. The PS2's renderer is read (re-point-lights 10); BF1's posts carry the same HoloOdf with friend GREEN (100,200,100) - read BF1's draw, then build both
milestone: M1
value: 5
phase: build
status: done
deps: BPNT
files: engine/crates/game/src/holo.rs,engine/crates/render/src/light_beam.rs,engine/crates/render/src/particles.rs,engine/crates/game/src/point_light.rs
declares: goldens=maps/bf1_bes1.png,maps/bf1_bes2.png,maps/bf1_end1.png,maps/bf1_geo1.png,maps/bf1_kam1.png,maps/bf1_kas1.png,maps/bf1_kas2.png,maps/bf1_nab1.png,maps/bf1_nab2.png,maps/bf1_rhn1.png,maps/bf1_rhn2.png,maps/bf1_yav1.png,maps/bf1_yav2.png,maps/bf2_cor1.png,maps/bf2_dag1.png,maps/bf2_dea1.png,maps/bf2_end1.png,maps/bf2_fel1.png,maps/bf2_kam1.png,maps/bf2_kas2.png,maps/bf2_mus1.png,maps/bf2_myg1.png,maps/bf2_nab2.png,maps/bf2_pol1.png,maps/bf2_tan1.png,maps/bf2_uta1.png,maps/bf2_yav1.png,maps/tat1.png,maps/tat2.png
needs:
effort:
branch: cphol
resume: landed dcfd692b 2026-10-13
created: 2026-10-12
alpha: yes
---

## Why
The maintainer, 2026-10-12 (chat, INBOX 19:05): "when is it going to fix the command posts to be the transparent hologram
type, blue for my team's cps and red for the enemy's?" On the PS2 a post is read from across the map by its hologram:
a translucent column in the colour of the VIEWER's relation to the owner, with the owner's team icon turning in it.
The port draws the post building and its light (card BPNT) and no hologram, so a post's owner cannot be seen in the
world. The spec sat in card LFLR beside the lamp flares and cones (value 4, in neither lane's next ten); this card
takes the hologram alone. LFLR keeps the flares, the cones and the EntityLight flicker.

## Evidence (all from `analysis/re-point-lights.md` 2 and 10, `[O]` unaudited; copied from card LFLR)
- **Renderer:** the post's hologram `FUN_002186c8` draws through the "LightBeam" renderer `FUN_00469628`
  (GS ALPHA 0x48 = additive, TEST 0x51001, untextured Gouraud, no Z write).
- **Beam:** 3.5 m tall, radius 0.5 -> 1.0 m, colour = the viewer's team-relation colour: own (1,86,213), enemy
  (200,100,100), neutral (192,192,192); alpha I x 0.25 x (1 - cos^2) (brightest at the silhouette's middle).
- **Icon:** the owner's `com_icon_*` model (texture `com_icon_controlzone`, translucent, no Z write), unlit, alpha
  I x 0.5, turning at 1.2566 rad/s (one turn in 5 s) at `HoloHeight` 3.5 m. A neutral post has NO icon.
- **I (intensity):** 2 s fades on a capture (`HoloTurnOnTime`), random pops and flicker dips. The post's LIGHT takes
  the same I (`game/src/point_light.rs` `command_post`: PROVISIONAL steady today).
- **Where:** at the post BUILDING's `hp_hologram` bone: 0.320 m up on `com_bldg_controlzone`, 1.363 m on the large
  post (`lead_tmp/b1dk/re/hp_census.py`, `analysis/re-kamino-decks.md` 6); the port's light constant is 0.32 for both.
- **PS2 frames that show it:** DEA1 cp3's red column, `pcsx2/ref/bf2_dea1/walk/pairs/cp3_cp3__sheet.png` b000 box
  (210,60,330,400): PS2 (94,68,74), port (35,37,43). UTA1 cp2 b180, the white column of a neutral post:
  `lead_tmp/bpnt/frames/u180__ctl.png`. The knock-out `lead_tmp/bpnt/frames/sheet_nopoint.png` shows the columns
  stay when every point light is patched out (they are draws, not light).
- **BF1** [H until read]: a BF1 post builds the same `HoloOdf` hologram (`FUN_0029c970`, re-kamino-decks 6) and its
  light's relation colours are friend (100,200,100) GREEN, enemy (200,100,100), neutral (192,192,192), a side-5
  locals team (223,223,32). Whether BF1 draws the beam and the icon as BF2 does has not been read.
- **In the bake already:** `engine/assets_baked/common_bf1/models/com_icon_{alliance,imperial,republic,cis}.glb`
  and `textures/com_icon_controlzone.ktx2` (checked by `find`, planner 2026-10-12). BF2's common bake: check first.

## Notes
- 2026-10-12 23:40 (planner): filed from the maintainer's INBOX note. **B1LL is NOT folded in:** it is BF1's omni LIGHT on
  every model draw (a bake change, 19 goldens, the 4th-power point weight) - a different renderer path and a
  rebake; this card is two additive draws. B1LL stays at 4 and shares only the bone height with this card.
- **A question put to the maintainer (STATUS 43):** on BF1's maps the PS2's own-team colour is GREEN, not blue. Default:
  faithful per game (BF2 maps blue / red, BF1 maps green / red). One constant if he wants blue everywhere.
- Value 5 by the maintainer's word ("soon"); lane 2 after ROLLS by the note. Undo: `loopctl.py set CPHOL value=4`.

- 2026-10-13 07:40 LANDED dcfd692 (goldens 0487827), report `reports/20261013_0740_card_CPHOL.md`. Open after the landing:
  cards ADDB (additive blend space: BF1's column reads 60 % of the PS2's) and STLC (still renders draw no COMMON-class
  building); PROVISIONAL: no beam where the fan's box reaches the near plane (fitted to 12 PS2 poses, re-point-lights
  15.5), BF1's update rate taken as 30 Hz, the icon's turn direction; not built: the flare, the icon's UV animation,
  END1's bunker panel offset and HOT1's shield-generator hologram, a live PCSX2 capture across a real capture.
