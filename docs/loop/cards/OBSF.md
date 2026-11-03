---
id: OBSF
title: A holder whose point lies beside a crate circles the crate's rim (OBST's early turn, the maintainer's Kamino scene: 17 reversals in 10 s): a holder's give-up counts the early turn as a pick that is not straight
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Card OBST (landed e9ce6db, 2026-10-07) walks a bot by a small body it sees up to 3 m ahead (`avoid::look_ahead`, `SteerPick::Round`). The landing's own check of the maintainer's F9 scene (BF2 Kamino, an idle human at -4.0, 60.3, 108.6; 8 seeds x 5 snapshots, `lead_tmp/obst/tools/scene_seeds.sh`) found holders on a 1 m step to a hold point that lies BESIDE a crate steering `round` for 10 s and more, along the crate's grown disc and back: unit 31 at (-12.9, 61.1, 125.3), hold goal (-14.08, 60.75, 125.66), 17 reversals, 4 m of path, 0.4 m net (`lead_tmp/obst/scene_final/t641800.json`). `Round` is not a blocked pick, so the holder's give-up (`HOLD_BLOCKED`) never ran. Trails over 6 reversals within 20 m: main before OBST 3 of 500 (worst 9), OBST 4 of 451 (worst 17).
