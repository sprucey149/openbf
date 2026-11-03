---
id: AITHK
title: AI think rate by distance and view: far or unseen bots think less often (the PS2's own rule), so a big bot count costs less CPU - deterministic, in the sim
status: archived
issue: 93
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #93](https://github.com/sprucey149/openbf/issues/93).

## Why
(the maintainer, 2026-10-19: speed on a weak laptop; bot count) Every bot thinks every tick. Engines save most of their AI CPU by letting bots that are far from the player or out of its view think less often. The PS2's own rule must be found (card SAIR / re-soldier-ai.md read the soldier AI; the think cadence by distance may or may not be there), because a made-up cadence changes the fights the gates measure.

## Evidence
`analysis/re-soldier-ai.md`, `analysis/re-tickets-spawn.md`, `analysis/bot-design.md` (the layers and the tick), `core/src/ai/` (the think call); the PC game's think cadence (every 4 / 3 / 2 / 1 / 0.25 s by distance and view cone) is a lead for the PS2 search, NOT the PS2's numbers.

## Acceptance
1. `analysis/re-ai-think-rate.md` [O] (or a section in re-soldier-ai.md): the PS2's rule with decomp lines, or a receipted statement that the PS2 has none.
2. If the PS2 has it: build it in `core` as a deterministic rule; gates: the sim hashes move only as declared, the fidelity gate and the sweep stay within tolerance, and ms per tick at 32 / 64 / 128 bots before and after is in the card.
3. If the PS2 has none: park the card with the finding; a modern-port variant (a presentation-only cadence for unseen far bots) is a proposal to the maintainer in STATUS, not a build.

## Notes

- MYGC (2026-10-22): value 4 -> 5; it is a fidelity card too, not only speed. The rule is already read and live-checked:
  `analysis/re-soldier-ai.md` 1.2 (interval = agent factor x {4, 3, 2, 1, 0.25} s by level; capture 2.0, combat 0.4 in
  state 2) and 1.3 (the level from the humans' distance, view cone and frustum; 0 with no human - the headless sweep).
  Acceptance 1 is therefore mostly a pointer to those sections plus the per-frame mover / tracker rate (1.1). A crude
  scratch knock-in at level 0 (`lead_tmp/mygc/rto_knock`, MYGC_THINK=240,48) with OPNW takes showcase bf2_myg1 from 3-13 to
  9-7 and its Alliance onto cp4 by 120 s in 8 of 8 rounds; it reshuffles other maps (all 32 x 16 seeds in
  `lead_tmp/mygc/allmaps.txt`), so the build referees every map and re-reads the PS2 columns, which are level 0 as well.
- RE (2026-10-22, lane 1, docs abd49ed): **acceptance 1 DONE** - `analysis/re-ai-think-rate.md` [O]. The PS2 has the
  rule (BF2 graded by level from the humans, BF1 a 2:1 camera skip); section 8 is the build spec. New beyond SAIR: BF2
  wakes a bot at the next step whenever a HUMAN-controlled unit enters / refreshes its threat list (seen, hit, or heard
  by the human's noise pass every 15 steps), at any level - live, every update, BF2 Kamino: 28 / 28 such entries pulled
  the think to now + 0.0001, 0 / 209 AI entries (`lead_tmp/aithk/ps2/`). A life's first order comes one interval after
  the spawn (4.0 s headless). Verifier workflow: 23 claims, 18 SUPPORTED, 5 PARTLY (folded in), 0 refuted.
- Build referee notes: the headless sweep is level 0 for every BF2 bot (trooper 240 ticks, fight 48, other 120) and
  BF1 30 ticks for slow units (the port's choice: no camera = far). Which PS2 columns are level 0: the bots-only
  captures with the player parked (WINS2, MYGC); BOTM's columns had the player spawned (`bot-quality.md`) - compare
  each with its own set-up. The build reverses bot-design D1 ("one rate for every bot"): MYGC asked the maintainer (default:
  the PS2's rule); the build updates D1's row.
