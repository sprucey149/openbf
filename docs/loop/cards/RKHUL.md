---
id: RKHUL
title: Rockets and bolts pass through walkers and some speeders: a vehicle hull's collision prims ignore their PRNT bone (game/src/assets.rs hull_parts, PROVISIONAL identity), so the spider walker's and AT-TE's hull sits at the feet while the body is 4-6 m up; BARC speeder 6/6 shots and STAP 2/6 miss too - the maintainer saw rockets pass through vehicles
milestone: M2
value: 5
phase: build
status: done
deps:
files: engine/crates/game/src/assets.rs, engine/crates/game/src/vehicle_pads.rs
declares: rebake=side; sweep
needs:
effort:
branch: rkhul
resume: landed a88147b8 2026-10-21; BARC/STAP not fixed -> card RKBARC
created: 2026-10-20
alpha: yes
---
## Why
Card RKEX's reproduction (2026-10-20, main 9f19c5a): the BF2 rocket (rep_weap_inf_rocket_launcher) fired from 5 / 20 / 60 m with a clear static line to each hull's blast-sphere centre, after 90 ticks of pad spawns. Hits on the target: cis_walk_spider 0/6 (the rocket lands on terrain 28-265 m behind), rep_walk_atte 0/3, rep_hover_barcspeeder 0/6 (KAS2), cis_hover_stap 4/6; rep_hover_fightertank 9/9, cis_tread_hailfire 6/6, cis_tread_snailtank 14/14. Soldiers, ground and walls stop it every time. The walkers' hull parts all sit within 3 m of the vehicle origin (spider: 11 parts, |t| <= 2.9 m; AT-TE: 20 parts, y ~ 0) while the body is 4.2 / 6.2 m up: `assets::hull_parts` takes each prim's PRNT bone as identity.
## Evidence
engine/artifacts/lead_tmp/rkex/repro_vehicles.txt; the scratch test engine/artifacts/lead_tmp/rkex/rkex_repro.rs (drop it into game/tests/suite, `mod rkex_repro;`, run with --ignored --nocapture, RKEX_MAPS="bf2_geo1 bf2_kas2").
