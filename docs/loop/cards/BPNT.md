---
id: BPNT
title: BF2's local point lights and lamp glows: UTA1's red lamp posts light a strong red pool and haze on the PS2 (floor (151,71,63) vs the port's (98,49,39)), its floors yellow-lit, TAN1's corridor pink, GEO1's bunker strips white-hot — the port lights with the sun, Light2 and the ambient only; RE the point-light path (lght local sets, the VU1 0x20 flag, which models take them) and build it
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
PXWB (2026-09-29, FINDINGS F48): BF2's local lights are a visible gap on the lamp-lit maps. The PS2 frames at the
walkabout's own stands: `engine/artifacts/pcsx2/ref/bf2_uta1/walk/pairs/cp2_con_cp1a__sheet.png` b180 (the red lamp:
floor (0,330,200,470) PS2 (151,71,63) vs port (98,49,39)), b000 (a yellow floor pool (80,71,51) vs (60,47,37)),
`cp5_con_cp6__sheet.png` b180 (walls (65,45,43) vs (46,35,34)), `cp3_con_cp1__sheet.png` b090 (a lamp halo (130,132,138)
vs (64,66,65)); `bf2_tan1/walk/pairs/cp0_cp6con__sheet.png` b270 (a pink wall (86,72,75) vs (52,55,58)); GEO1's bunker
light strips (`bf2_geo1/walk/pairs/cp4_cp4__sheet.png`).

## Evidence so far (read, not yet RE'd for the draw)
- DEA1, FEL1 and UTA1 ship local point-light sets in `lght` besides GlobalLights (`analysis/re-sky-lighting.md` §1).
- BF2's VU1 lighting takes point lights (flag 0x20), and a prelit model gets COL0 plus the NON-Static lights only
  (`analysis/re-model-lighting.md` §2; EE uploads FUN_00497278).
- The port's model lighting is the sun, Light2 and the hemisphere ambient (`bake/src/lighting.rs`); TDYN is the
  terrain's twin; GLOW owns the self-lit halos.
- TAN1 has no point-light set: its pink wall is [H] a light region (LREG) or COL0.

## Notes
- 2026-09-29 (PXWB part 2, FINDINGS F55): DEA1 (a map with a point-light set) — a red glow column at cp3's console (`pcsx2/ref/bf2_dea1/walk/pairs/cp3_cp3__sheet.png` b000 (210,60,330,400) PS2 (94,68,74) vs port (35,37,43)).

## Note (2026-09-30, FXB RE, `analysis/re-bolt.md` §5.4, from helper C; the model path is this card's)
BF2 models (static and skinned) gather ≤ 4 omni lights by the same list and score (`FUN_00597e98` from `FUN_0057cf60`
and `FUN_001de2a8`; sphere test |c − L|² < (r + R)²); a non-Static light reaches prelit buildings too; per vertex
colour += C·mat·((1 + N·L̂)/2)²·(1 − min(1, d²/r²))² (VU1, a fast rsqrt ≤ ~9 % high). BF2 bolt lights are such
non-Static omnis (card FXBL builds their terrain pass) — once this card lights models from runtime omnis, bolts light
the soldiers they pass. BF1's point term (its explosions' LightFlash) is ((1 + N·L̂)/2)⁴ (AGENT_B §2.2).

## RE result (2026-09-30, lane 3; `analysis/re-point-lights.md` [O]; report `reports/20260930_1640_card_BPNT.md`)
- **Sources** (§1–§4):
  - every non-stub `lght` chunk (all top-level, so every mode loads them; mostly Static; `PS2Overbright` packs the
    colour ×255 instead of ×128, i.e. twice as bright on models — the port's `mungefs::light` drops it as editor-only);
  - EntityLights attached to props by `AttachOdf` (UTA1 26, HOT1 88, KAM1 45, TAT3 120; OmniRadius 0 = no light);
  - the CP hologram's omni: r 8 at the CP + 0.32 m, never Static, in the viewer's team-relation colour — own
    (1,86,213), enemy (200,100,100), neutral (192,192,192);
  - bolts, explosion LightFlash, flags / mines.
- **Takers** (§5–§8): every model draw (static and skinned) gets ≤ 4 omnis + ≤ 1 spot (as a point, no cone) by
  score = lum·min(1, r²/4d²)/max(1, e²/3600). Prelit models and the terrain skip Static lights. Per vertex:
  C·mat·((1 + N·L̂)/2)²·(1 − min(1, d²/r²))².
- **TAN1's pink** = the enemy CP's red light on a prelit wall (not LREG / COL0).
- **UTA1's red pool** = the lamp post's authored Static red spot on the (non-prelit) floor model.
- **The yellow floor and soldier** = 33 Overbright yellow spots.
- **GEO1's strips** = GLOW (landed).
- **The red haze at the lamp heads** and **DEA1's red column** are NOT light: they are lens flares / light cones / the
  CP hologram's beam → new card **LFLR**.
- **Acceptance boxes**, PS2 knock-outs (SW renderer, `lead_tmp/bpnt/frames/`):
  - UTA1 cp2_con_cp1a b000, floor (380,340,640,480): PS2 102.6/93.0/55.1 (library HW 106.3/96.0/55.7). With no model
    point lights it is 58.5/51.3/45.8 on the PS2; the **port today is 57.1/46.4/39.6** → the build must reach the PS2's
    value within ~10.
  - TAN1 cp0_cp6con b270, the wall: the pink the PS2 loses without model omnis is (0,230,640,330) +17.4/+8.3/+8.3 → the
    port must gain it.
  - UTA1 cp3_con_cp1: the player gains +0/+7.9/+16.2 blue from his CP.
  - Plus the card's own PXWB boxes, and PARITY on UTA1 / TAN1 not worse.
- **For the builder** (reversible calls):
  - Per vertex, as the PS2 does: its pools follow the mesh's vertices. Per-pixel would draw hot spots the PS2 never
    shows. Say so in FEEL_NOTES if you choose otherwise.
  - Light colours through the same sRGB → linear as LITEB's BF2 sun.
  - Keep the runtime list open for FXP5's explosion LightFlashes. BF1's only point source is that flash, and its model
    term is ((1 + N·L̂)/2)⁴ (`re-bolt.md` §5.4).
  - TDYN is closed into step 4.

- Planner 2026-09-30 17:30: Raised 4 → 5: the RE is done with measured boxes; UTA1 (showcase) floor 57/46/40 vs the PS2's 103/93/55 is one of the largest colour gaps on a showcase map, and it gates LFLR.
