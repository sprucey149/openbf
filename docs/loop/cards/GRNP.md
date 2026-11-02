---
id: GRNP
title: The grenade's throw against PS2 BF2 (the maintainer, 2026-10-04: 'the port feels like it throws further'): measure distance, arc, bounce, roll and stick in PCSX2 and in the port from the same stance and pitch, then make the port's the PS2 BF2's
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/sticky.rs,engine/crates/core/src/sim/ordnance.rs,engine/crates/core/src/collision/rays.rs,engine/crates/core/src/weapon.rs,engine/crates/game/src/roster.rs
declares:
needs:
effort:
branch: grnp
resume: landed fdff50d9 2026-10-06
created: 2026-10-04
alpha: yes
---

## Why
The maintainer, 2026-10-04: "Compare grenade throw distance with the original ps2 games - the port feels like it is throwing grenades further than the original but maybe I'm wrong on that. I want grenade throw distance and physics to be approximately the same as the ps2 bf2 grenade physics in how it throws, distance, sticks, and bounces and such."

## Evidence
- GREN (done) built the throw from the disc's keys (release 0.30 s BF1 / 0.51 s BF2, ShotElevate 0.21). Nobody has set a PS2 throw beside the port's.

## Notes
Filed 2026-10-04 by Claude in chat from the maintainer's playtest (build 1bde70c, main 4e51e84). The maintainer's own words are quoted; nothing here is measured unless it says so.
- 2026-10-06 planner: value 4 -> 5, for the same reason as SPRD (the maintainer's own ask of 2026-10-04, not reached at value 4). A PCSX2 measure: `ps2live.py` / the scripted-pad probes (memory note "PS2 live samplers and pad probes": ordnance rows) give the grenade's position per game update; card GRAI (the AI's grenade, after FGHT) reads this card's table for its low-arc solution, so say on GRAI what changed. Undo: `loopctl.py set GRNP value=4`.
- 2026-10-06 20:41 measure session (lane 2; report `reports/20261006_2041_card_GRNP.md`): the table above. The maintainer's
  impression holds only for the roll: thrown level or downward the port rests 0.9-1.2 m (3-7 %) further and every
  throw goes off 0.17-0.23 s sooner; the throw itself is the PS2's. Cause (RE, verifier 9 of 9): BF2's terrain ray
  cuts a ray at the first bisection midpoint within 0.1 m of the ground, not at the crossing (§13.2) — the port takes
  the crossing — plus a second difference in the port's own sweep (9 contacts where the PS2 makes 8; the exact-crossing
  transcription rests at 35.87 m, the port at 36.57): find it with the fixture, not by reading.
- Build decisions taken here (reversible; in the report's Decisions): (1) the bisection goes into the STICKY's terrain
  test only — bolts and the camera keep the exact crossing (0.1 m on a bolt's impact is invisible; the PS2 uses the same
  ray for them); (2) on a BF1 map the thrown ordnance takes BF2's THROW rules — the hand launch point, the 0.51 s
  wind-up, the slow-bounce rest rule, LifeSpan 0.6, the stick mask (vehicles + buildings) — and keeps BF1's damage,
  radius and ammo counts (the maintainer: "how it throws, distance, sticks, and bounces"); one const / flag to put BF1's back.
- Not for this card, noted: the aim's upper limit is 63.62 deg on the PS2 against the port's 1.1 rad (63.03); the
  PS2's chase camera looks 10.62-10.67 deg below the aim, the port's 10.00 (`camera_section` tilt) — check against
  `re-camera` before touching; the port's snowspeeder hull stops a grenade 0.81 m further along than the PS2's
  (BF2 HOT1 hangar, (-511, -96.6), throw from (-511, -84) aim -9.1 deg: 10.92 m / 11.73 m).
- Tools: `lead_tmp/grnp/ps2/grnp_probe.py` (PS2 throws at a site / yaw / pitch list), `tools/grnp_throw.rs` (scratch
  example; built in `openbf-wt/grnp-scratch`, target `lead_tmp/grnp-scratch_target`), `tools/compare.py [--reuse]`,
  `tools/replay.py [--exact] [-v]`, `tools/flat.py`. GRAI reads the launch (unchanged) — nothing in GRAI's low-arc
  solution moves; a bot's grenade will land with the same bisection once it throws.
- 2026-10-06 21:16 build session (lane 2; report `reports/20261006_2116_card_GRNP.md`): **LANDED fdff50d.** Acceptance, run:
  (1) `sticky_ps2_bf2_throws_match_row_for_row` — 7 PS2 throws, 958 rows, worst 0.5 mm, rest and blast updates equal;
  fails on main 66e15a9 (level throw 0.285 m off at update 33) and under two mutations (exact crossing: 0.253 m;
  the old +0.3 ray: 0.075 m). (2) `tools/compare.py` re-taken at fdff50d (`ps2/out/tat2_a_compare_branch.txt`): 14 of
  15 at the PS2's rest point and update; the 15th (aim limit 63.0 / 63.62 deg, noted above as not this card's) rests
  on the PS2's update 0.67 m long. (3) `bf1_nab2` thermal, level: 35.49 m / 2.467 s (`port_bf1_nab2_branch.txt`).
  Found on the way: the "second difference" was the sweep ray 0.1 m too long; and the PS2 soldier rests 2 cm into the
  floor, so the hand is 1.7983 m over the port's feet (re-grenades 13.4). Gates: nothing moved (no bot throws yet).
  Left for the planner: the aim's upper limit (63.03 -> 63.62 deg) and the chase camera's 10.65 deg tilt are still
  un-carded notes; BF1's own terrain ray was not read (the port takes BF2's everywhere, by the maintainer's ask).
