---
id: B1DK
title: Kamino's decks after LSUM: BF2 makes an EntityLight whose hardpoint the carrier lacks at the carrier's origin (Kamino's bunkers: 10 lights of r 20 in the middle of the decks; POL1 2, DEA1 1) and the bake skips them; BF1's sun-coloured specular pass is fed the linear sun colour (3-5 x too weak under Kamino's sun)
milestone: M1
value: 5
phase: build
status: done
deps: LSUM
files: engine/crates/bake/src/entity_light.rs,engine/crates/bake/src/map.rs,engine/crates/game/src/assets.rs,engine/crates/game/src/viewer/scene.rs,engine/crates/game/src/soldier_vis.rs,engine/crates/game/src/viewer/vehicles.rs,engine/CRATES.md
declares: goldens=maps/bf2_kam1.png,maps/bf2_pol1.png,maps/bf2_dea1.png,maps/bf2_uta1.png,maps/bf1_kam1.png,maps/bf1_kas2.png,maps/bf1_yav1.png,units/bf1_all_inf_pilot.png,units/bf1_cis_inf_bdroid.png,units/bf1_cis_inf_sbdroid.png,units/bf1_imp_inf_darktrooper.png,units/bf1_rep_inf_pilot.png,units/bf1_rep_inf_trooper.png,walkabout_stats.ron; rebake=maps
needs:
effort:
branch: b1dk
resume: landed 71c38556 2026-10-05
created: 2026-09-30
alpha: yes
defer: 
---

## Why
BF1 Kamino is the worst map of the parity board (26.62) and its floors are the largest term after the sky (KSKY) and
the sea (KSEA): the landing-pad decks read 1.5–2 × darker than the PS2's, uniformly over RGB, at every bearing.
From KAMR's pass, `analysis/re-kamino.md` §5.

## Evidence
- Deck box (450,330)-(640,470) at the stored walk poses, main 5c73167 (`lead_tmp/kamr/box.py`, renders
  `lead_tmp/kamr/parity_main_5c73167/render`): cp0_cp2 b000 PS2 (65,79,83) / port (41,46,52); b270 (85,102,116) /
  (43,49,58); cp4_cp4 b000 (73,82,92) / (42,46,52); b270 (85,102,116) / (43,49,58). The port's deck does not change
  with the bearing at all (41–43); the PS2's does (61–85, 148 where a highlight sits).
- BF2's same decks were 1.7 × dark until BPNT landed BF2's local lights (3c024fb): now (72,76,81) vs the PS2's
  (72,78,86) at cp2_cp3 b000. BF1 has no `lght` chunk; `analysis/re-point-lights.md` is BF2 only.
- BF1 KAM1 defines `whitelight` / `kamlight` / `kamlight_blue` (base `Light`) but the baked world places none; the deck
  material's `spec_pass` bakes `rgb: null` on BF1 (`rtyp: Specmap`, power 4).

## What to find
Which term lights BF1's decks: the Specmap second pass (does the port draw it on BF1 at all — `rgb: null`), a BF1
local-light path (the `Light` classes, `SetAttached…`/world lights in BF1's world file), or the DomeInfo filter
(BRGT). One PS2 knock-out per candidate on a `bf1_kam1_walk_*` state settles it.

## Note from card BPRL's RE (2026-10-03, lane 3)
The title's "BF2's matched once BPNT landed its local lights" does not hold. BF2 Kamino's deck is dark like BF1's: at
`bf2_kam1 walk/cp2_cp3_b090` the PS2's deck is 82/84/89 at mid-floor (box (256,278,627,346)) and the port's 45/48/51.
Near the camera (352,360,627,470) the port had 82/86/91 against the PS2's 80/84/93 only because the post's light drew
a perspective-blended pool there, which the PS2 does not show (`analysis/re-vertex-colour-interp.md` sections 4-5);
with the PS2's blend (BPRL's build) that box is 53/57/62 and bf2_kam1's parity rises 16.05 -> 16.78. So the deck
term this card is after is missing in BOTH games, and BF2's PS2 states (`bf2_kam1_walk_*`) can serve the knock-out.
Planner: widen the title to both games; BPRL's build declares `parity=bf2_kam1` against this card.

## Planner 2026-10-03 07:15 - both games, value 4 -> 5, alpha, no longer post-alpha
- The title is widened as BPRL's RE asked: the missing deck term is missing in BOTH games. BPRL's landing (b9c7939)
  took away the perspective-blended pool that had hidden it near the camera on BF2, so BF2 Kamino's deck is now dark
  where the player stands (bf2_kam1's parity rose 0.655 at the landing, declared against this card, and stands at
  16.71 in `loop/parity/summary.md`; `reports/20261003_0555_card_BPRL.md`).
- Why 5 and alpha, against the 04:30 planner's deferral: that note deferred it as BF1-only lighting parity off the six
  showcase maps. It is now a floor 1.8 x too dark on the map the maintainer plays (INBOX 2026-09-30 22:50: the Kamino cards go
  before value-5 cards on other maps), and it got visibly darker tonight.
- Use BF2's states for the knock-out (`bf2_kam1_walk_*`); one PS2 knock-out per candidate, as "What to find" says.
- Acceptance, added for BF2: the mid-floor box (256,278,627,346) of `bf2_kam1 walk/cp2_cp3_b090` within +-12 levels per
  channel of the PS2's 82/84/89 (port 45/48/51 in BPRL's RE; re-measure first); `parity.py` bf2_kam1 down by 0.66
  or more from 16.71 (BPRL's declared rise taken back); BF1's lines stand.
- Reverse: `loopctl.py set B1DK value=4 alpha= defer=post-alpha`.
- 2026-10-05 (card LSUM landed, bf263f8): the light sum is now made in display units and the ambient floor is gone (report `reports/20261005_1505_card_LSUM.md`). LSUM is on main: this card is unblocked. Form 1 was kept (form 2 scored 10.052 against 9.742).
