---
id: REACT
title: A bot that is being hit reacts instead of standing there (the maintainer's playtest 2026-10-07): per hit, a chance by difficulty to stay, strafe, dodge-roll, or move to a new angle or cover - for a fighter in the stand style too
milestone: M3
value: 5
phase: build
status: done
deps: FGHT
files: engine/crates/core/src/ai/behave.rs, engine/crates/core/src/ai/state.rs, engine/crates/core/src/skill.rs, engine/crates/core/src/ai.rs
declares: sweep; arena; sim
needs:
effort:
branch: react
resume: landed 19708fad 2026-10-12
created: 2026-10-07
alpha: yes
---

## Why
The maintainer, 2026-10-07: "when you are shooting at a bot, if it doesn't die after being hit, it just stays standing there. In the original games there was a chance when you were shooting at a bot that it would either stand still, or try moving around some. The bots do a better job at spreading out, but now they are mostly just standing there. This can also go into the difficulty ladder, but the bots should move around and dodge more and be more aware they are in a firefight and react accordingly when the difficulty gets harder. So there is a percent chance they stay where they are, or they do something such as strafe while fighting, or dodge/roll, or move to a new position or something for a new angle or cover."

## Evidence
- **What the PS2 does** (`analysis/re-soldier-ai.md`, the fight row; read by the loop, not by the maintainer's memory): a bot's engagement MODE is its own fixed number, drawn once: stand and shoot (a third of those crouched), circle at the weapon's optimal range, zig-zag (while the target aims at it within 18 deg), flank. 'A damage message is ignored in states 5 and 6' - a bot in the stand mode does NOT react to being hit; the moving looks come from the other bots' modes. The level's weights: Easy {3,1,1,1}, Normal {2,2,1,1}, Elite {1,1,5,1} (zig-zag x5).
- **What the port does since FGHT (landed 10-07):** half stand to shoot (a third crouched), half circle; zig-zag and flank are not built (FGHT2 / FGHTA); the on-hit dodge exists only for a HOLDER (card HOLD: a 2.5 m step or the tier's roll) and the fight replaced hold for a bot that sees a target. So a fighter in the stand style stands whatever hits it.
- **What the design already has, unbuilt:** `analysis/bot-design.md` section 7's tier table: evasive step when aimed at 10 / 30 / 50 / 65 %, roll when hit 20 / 35 / 50 / 60 %, leaves a compromised position never / after 8 s / after 3 s / at once, falls back when hurt (Hard, Elite); the maintainer's BOTD answer 3: a peek cycle and a new position every 5-12 s (card TPOS).

## Notes
Filed 2026-10-07 by Claude in chat from the maintainer's playtest (BF2 Kamino, F9 at tick 10701, build 6f6160b; snapshot `playtest/prev1.json`). His words are quoted; nothing here is measured unless it says so. Order: it can be built on today's strafe and roll steps; TPOS gives it the position step, TIER / TCMP the per-tier columns. Related: FGHT2 (zig-zag, flank, the close contact), BOTV (a bar for it).
- 2026-10-09 (card FGHT2's session; FGHT2 now waits on the maintainer): **zig-zag and flank are this card's to build or drop**
  - FGHT2 measured the contact gap and did not build them: they are modes inside a fight (the PS2's attack rows on
  Kamino: circle 48 %, stand 23 %, cover 21 %, flank 5 %, zig-zag 2 %; `lead_tmp/audit-fght/README.md`) and do not
  bear on how near the bots get. A caution from the same measure (`analysis/re-soldier-ai.md` 15.5): on the PS2
  45-70 % of all damage lands on a BF2 bot that is NOT in a fight order (it keeps walking until its next 4-8 s
  think), the port's 2-7 % - "a bot that is hit reacts" is already truer of the port than of the PS2, and the maintainer's
  ask goes further from the PS2, as a named deviation. Tool: `lead_tmp/fght2/tools/aware.py`.
- 2026-10-11 (build session, lane 2; report `reports/20261011_1549_card_REACT.md`): **BUILT on branch `react` 2229b14, ci green.**
  - The Evidence line "a fighter in the stand style stands whatever hits it" was partly stale: a fighter carries a
    `HoldState`, so card HOLD's on-hit dodge did apply to it - at the PS2's coin (7 % Easy, 17 % Normal, 20 % Hard /
    Elite per hit, 2 s apart), and its "roll" never travelled (a standing soldier rolls on the spot: no impulse under
    0.1 m/s). Measured on main 8ad30f4 before any change (`HIT_CENSUS=1`, 8 seeds x 32 maps per tier): of a fighter's
    seconds within 2 s of a hit, standing (< 0.5 m/s) 75.9 / 60.0 / 55.8 / 54.3 % - the maintainer's "51 % standing" holds.
  - Built: `skill::Tier::react` (stay 65 / 35 / 20 / 10 %; strafe 20 / 30 / 30 / 30; roll 5 / 15 / 20 / 25; move
    10 / 20 / 30 / 35), `behave::react`, `FightState::MOVE`, the roll pressed 6 ticks after the bot sets off, no
    holder's step through a death region, a roll or a move ends in a hub of the graph.
  - **Acceptance, measured** (`lead_tmp/react/sw/{base,r5}_<tier>`, `tools/fire.py`; fighter-seconds under fire):
    standing 75.9 -> 67.6 % Easy, 60.0 -> 49.5 Normal, 55.8 -> 43.1 Hard, 54.3 -> 40.0 Elite; inside a 3 s spell
    within 1.5 m 18.9 -> 14.6, 10.0 -> 6.3, 7.3 -> 3.9, 6.8 -> 3.3 %; 1.5 m away 2 s after a burst's first hit
    41 -> 52, 63 -> 74, 68 -> 81, 69 -> 85 %. BF2 Kamino at Normal: standing 63.6 -> 52.7 %, 3 s spells 14.0 -> 8.3 %.
    256 of 256 rounds finish at every tier; deaths to the kill floor 229 -> 220, in death regions 68 -> 93 (seed
    noise: 34 and 77 on two base blocks of the five maps that have them), recycles 225 -> 195.
  - Tests (`ai::behave`): the row's shares over 400 hits a tier, the cooldown, 20 s of steady fire, the roll's
    travel, the death-region rule; each fails under its own mutation (5 of 5).
  - **Zig-zag and flank: dropped here** (the 10-09 note made them this card's to build or drop): 2 % and 5 % of the
    PS2's attack time, 12.5 / 17.5 m of open ground each; the reaction is what answers the maintainer's words. To reverse: a
    new card on `fight_position`'s style draw.
- 2026-10-12 09:52 land BSPN: BSPN landed (main 2d1fb60) on an approved `stand20_pct@bf2_dag1` 0.36 -> 1.08, and `[fidelity]` still holds the 8ad30f4 baseline. Rebased on main this branch will read that move as its own until the planner re-takes the baseline - a refusal on that one number is not this card's.
