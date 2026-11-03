---
id: WTHR
title: Weather, precipitation: rain on both Kaminos and BF1 Yavin 2, snow on the Hoths and Rhen Var, ash on Mygeeto, embers on Mustafar, pollen on Felucia - the PS2's particles in one 5 m cube tiled round the camera, streaks as lines, flakes as sprites, wind, rain shadows and splashes (RE done: re-weather.md)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## The maintainer's decision (2026-09-25)
In scope: rain and lightning are the disc's own content, and anything the PS2 draws is in scope without asking. Build it faithful-yet-modern.

## Why
The PS2 draws precipitation on nine maps - rain on both Kaminos and BF1 Yavin 2, snow on both Hoths and BF1 Rhen
Var, ash on BF2 Mygeeto, embers on BF2 Mustafar, pollen on BF2 Felucia - and the port draws none. The maintainer plays Kamino;
Hoth and Mygeeto are showcase maps. THIS CARD IS NOW THE BUILD OF THE PRECIPITATION (RE done 2026-10-05); the
lightning flashes, thunder and the sky bolt are card WTHL.

## Evidence
`analysis/re-weather.md` ([O]; verifier 64 SUPPORTED / 5 PARTLY folded in / 0 NOT of 69). The whole rule set:
- sections 1-2: the nine blocks, the field map, N particles in ONE unit cube tiled over the world in 5 m cells, the
  0.5 s re-roll, the motion (fall + wind + offset);
- section 3: a streak = one line from the drop (its alpha) to where it was StreakLength/30 s ago relative to the
  moving player (alpha 0), 1 pixel of the PS2's 512, colour = Color applied twice (Kamino (91, 94, 101) in display
  units), ordinary alpha blend, depth tested, no depth write, not fogged; a flake = a camera-facing textured square
  of edge 2.83 x floor(ParticleSize x 256)/256, spun by its phase, at its own alpha with NO distance fade;
  the cells drawn and a streak cell's alpha (dpx, Range, the 0.66 fade);
- section 4: the `rainshadow...` regions (4-23 a map) and the cell walk that keeps the rain out of halls;
- section 5: the splashes (`com_sfx_rainsplash` / `rainsplash`, already baked) and their rate; section 6: the wind.
Instruments: `lead_tmp/wthr/tools/predict.py <state>` (rebuilds a PS2 frame's streaks from that state's RAM and
scores them against the state's screenshot; `--export` writes a test vector), `flake_fade.py`, `ramcensus.py`,
`regions.py`, `census.py`. Vectors: `lead_tmp/wthr/vectors/bf2_kam1_cp1_cp6_b000.json` (a rain-shadow view, 27 cells),
`bf2_mus1_cp0_cp5_b000.json` (a plain view, 28 cells).

## Build notes (decisions taken in the RE; reversible)
- Viewer-side, not sim state: its own LCG stream, frame dt; the sim hash must not move (`declares:` has no sim).
- Streak width: the PS2's line is 1 pixel of 512; draw a streak 1/512 of the frame's width (>= 1 px) so the look
  holds at any resolution - one FEEL_NOTES line.
- Flakes keep the PS2's missing fade (they pop with their cell at Range, 15-20 m, at 3-8 cm: not visible); a fade
  would be a named deviation - not taken.
- The streak pass needs a per-vertex alpha (head a, tail 0): `render::ParticleDraw` has one colour per draw.
- BF2 divides GroundEffectsPerSec by the view count, BF1 does not; the PS2's camera-velocity input is the player's
  soldier / vehicle velocity.
- Goldens: decide in the build whether the still goldens show weather (a fixed particle state at t = 0 keeps them
  deterministic); `declares:` below assumes they do on the nine maps and that every other map is untouched.

## Original placeholder (kept)
See the original entry below.

## Original QUEUE.md entry (verbatim, 2026-09-25)
WTHR. [TODO — ADDED 2026-09-25 (lane 1) by PXC batch 2 (FINDINGS F11); **[RE] FIRST**; FREE; LOW-MEDIUM value; a build is a new presentation system — check PLAN §LOCKED FEATURE SET §Presentation first and record the call] **Weather: rain streaks and lightning bolts on Kamino (BF1 and BF2) on the PS2, none in the port** [RE] — evidence `engine/artifacts/pcsx2/ref/bf1_kam1/pairs/cp0_cp2_beta1_v3_water.png` (two bolts) and the rain in every BF1/BF2 KAM1 frame; PLAN has no weather text (0 hits for weather / lightning / precipitation — the batch-2 verifier). The maintainer's `engine/artifacts/pcsx2/ref/COMPARISON_RULES.md` counts a missing layer that changes the game's character as a finding. RE (§5): what authors it — the map's `fx__` config (an `Effect("Rain")` / lightning entry beside `Effect("Water")`, which `mungefs::fx` parses), the streak/particle system, the flash, its sounds — and a census over both discs. GATE: `analysis/re-weather.md` (§5, [O], verifier) with the census + a build entry. Files: `analysis/re-weather.md` (new), `analysis/RE_LOG.md`, scratch `engine/artifacts/lead_tmp/re/wthr/`.

## Notes (planner 2026-09-25)
- The RE may run; the BUILD waits on the maintainer: PLAN's LOCKED FEATURE SET has no weather, and adding it is his call (STATUS
  For the maintainer). Do not set this card to phase=build until INBOX answers.

## Note (PXWA, 2026-09-29, lane 2): BF1 Rhen Var snows
The PCSX2 walkabout shows falling snow on BF1 RHN1 (and flakes on RHN2) at every post; the port draws none. RHN1.LVL
authors `Precipitation` / `fx_snow` / `rhn1_snow_01` (disc strings). Frames: `engine/artifacts/pcsx2/ref/bf1_rhn1/walk/
pairs/cp0_cp2__sheet.png`, `cp4_cp5__sheet.png`. The snow may also be part of RHN1's near haze (FARC note).
- 2026-09-29 (PXWB, FINDINGS F56/F60): BF2 too — heavy rain at every outdoor KAM1 post (`pcsx2/ref/bf2_kam1/walk/pairs/cp1_cp6__sheet.png`), falling snow on HOT1 (`bf2_hot1/walk/pairs/cp4_cp7__sheet.png` b090/b270), ash on MYG1 (`bf2_myg1/walk/pairs/cp0_cp4_con__sheet.png`).

## Note (KAMR RE, 2026-09-30, lane 3): the Lightning effect is read — use it
`analysis/re-kamino.md` §1: the disc block (BF1 KAM1, BF1 YAV2, BF2 KAM1: Lightning + LightningBolt("skybolt") +
Precipitation "Streaks" + Wind + ColorControl; `lead_tmp/kamr/re/fxkeys.py <LVL>` prints every key), the parser and the
tick (B2 `FUN_0045aa30` / `FUN_0045ae80`, B1 `FUN_00377c40` / `FUN_00378120`): flash schedule (interval 3..5 s, 2..5
sub-flashes 0.01..0.5 s apart, a random direction 30..60 deg over the horizon), FadeTime 0.2 s, and per tick of a fade
the lights blended toward the flash colour, the dome colour AND the fog colour lerped white -> black by
0.4*(1 - brightness). PS2 frames at a flash: `lead_tmp/kamr/ps2exp/out/sea_ctl2.png`, `sea_dome255.png` (bolt, light
grey fogged sea edge). The steady dome factor is card KSKY (it bakes the block: take the manifest field from there).
The Lightning object in RAM: BF2 KAM1 0x173a750 (float pattern 0.1, 0.4, 1.0, 0.2, 3.0, 5.0 at +0x34); set +0x68 = 1e9
to freeze it for clean frames.

## Planner 2026-10-03 07:15 - value 4 -> 5
- Why: Kamino's rain and lightning are on the map the maintainer plays, the card has been at 4 since 09-30 and the research
  lanes take equal-value RE cards in ID order, which put it last of eleven. The maintainer's rule of 09-30 22:50 (Kamino cards
  before value-5 cards on other maps) names it by its KAMR note. HOT1's snow and MYG1's ash are two showcase maps.
- The Lightning block is already read (the note above); what is left to read is the Precipitation "Streaks" draw.
- Reverse: `loopctl.py set WTHR value=4`.

## Note (WTHR RE, 2026-10-05, lane 2)
- RE done, docs 6927748. The "Acceptance (planner 2026-09-30 17:30)" section above is superseded by "## Acceptance (run
  and paste each)" at the top: the pixel-count measure it proposed is replaced by the RAM vectors and the frame test
  (a model scored against the PS2 screenshot of the very state it was read from).
- BF1 RHN2 has NO precipitation (disc and RAM); the "flakes on RHN2" of the PXWA note are not this effect.
- The lightning half of this card's title is card WTHL.
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.

## Note (WTHR build, 2026-10-06, lane 2) - LANDED 1dbb18f + goldens 9cd9b52
- Report `reports/20261006_2350_card_WTHR.md`. Acceptance 1-4 run and pasted there: three PS2 vectors (27 / 28 / 4
  cells, alpha gap under 6e-6), the frame test 0.444 at cp5_cp5 (shuffled 0.017, indoor poses 0 streak pixels), five
  mutations each failing a test, pairs in `lead_tmp/wthr/out/pairs/`.
- `declares:` was narrowed from `goldens=any` to the nine `maps/*.png`; all nine moved and nothing else.
- Left for a later card if wanted: the flake maps' parity rose 0.03-0.15 (cause not found); the splash loop's second
  writer of the "in view" flags; one splash seen over the sea edge (surface not identified).
