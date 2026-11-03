---
id: AICLS
title: Which class an AI spawn takes on the PS2: the assignment from the AddUnitClass counts / SetupTeams minimums, and whether a dead bot re-rolls its class — the port's next_ai_class and keep-your-class rule are PROVISIONAL (ours)
status: archived
issue: 9
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #9](https://github.com/sprucey149/openbf/issues/9).

## Why
Cleanse CLNC (2026-09-30): two PROVISIONAL rules in `core/src/sim/spawn.rs` named an open RE question ('HOLD FOR FABLE') that no card or analysis section owned. The unit mix on the field is player-visible on every map (how many rocketeers, snipers, pilots a team fields, and whether the mix drifts over a battle).

## What to find
- The spawn path's class pick in both games (BF1 around FUN_00295c40's AI spawn gate, BF2 FUN_0034cea8; re-tickets-spawn.md has the gate, not the pick): how the per-class count / min / max from AddUnitClass / SetupTeams become the next spawn's class.
- Whether a respawning AI keeps its class or is re-picked.

## Acceptance
- The rule [V] in both games (re_check fn / callers / word), in `analysis/`.
- A PS2 census: `ps2live.py` (BF1) / the BF2 soldier readers (`lead_tmp/unit-audit/*2.py`) count live AI per class per team at 0 / 2 / 5 min on one map each; the rule predicts the counts within 1 per class.
- Then phase=build: the port's per-class live counts on the same maps match the PS2 census within 1 per class (a sweep line).

- Fable 2026-10-01: `Sim::ai_class_weights` (023e3fd) replaces the minimums walk and keep-your-class. The maintainer's follow-up: the default rifleman highest, special classes lower, every class keeps a floor.

- Fable 2026-10-01 (second session): AICLW e4b8d6e - the shares are 4 : 2 : 1 (default : standard : special) by the disc's class order, each living bot one unit off, the same floor for every class. The PS2 RE question is unchanged.
