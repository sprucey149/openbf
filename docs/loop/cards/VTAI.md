---
id: VTAI
title: Bots use seat guns: a bot pilot fires its seat's weapon and a bot may claim a gunner seat; its look/aim from brain's target through the same perception / fairness rules as on foot
milestone: M2
value: 5
phase: build
status: done
deps: VTUR
files: engine/crates/core/src/ai/gunner.rs,engine/crates/core/src/ai/ride.rs,engine/crates/core/src/sim/guns.rs,engine/crates/core/src/perception.rs,engine/crates/core/src/ai/brain.rs
declares: sweep
needs:
branch: vtai
resume: landed 744f8f8; turrets → VTAT
created: 2026-09-29
---
## Why
VAI's bots board and drive but fire nothing; VTUR (2026-09-29) arms only HUMAN riders (core::sim::guns::Sim::seat_gun returns for a bot). A vehicle full of bots that never shoot is the biggest remaining AI gap in M2.

## Evidence
VAI note on VTUR: ride::drive and a gunner seat's think must aim and press FIRE through brain::think's fire block (perception + the fairness assert in step 4); ride::claim takes the pilot seat only. re-vehicles §5 (AI boarding: BF1 FUN_00244848 → FUN_0023c418; BF2 AI commands 0x13/0x14) — the PS2's AI gunner logic is not yet read [H].
