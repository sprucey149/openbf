---
id: JWAL
title: Soldiers jump through solid walls (the maintainer's play session 2026-09-29): on BF1 KAS2 a jump at a round platform's bamboo side wall carries the soldier inside the hollow under the deck — find the mechanism (sweep tunnelling at jump speed, a seam or gap in the baked collision, or the PS2's own hole), close it, a replay test
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/collision/walls.rs,engine/crates/core/src/sim/step.rs,engine/crates/core/src/collision/shape.rs,engine/crates/core/src/ai/steer.rs,engine/crates/game/tests/jwal_cityhall.rs,engine/crates/game/examples/leak_census_accepted.txt,engine/CRATES.md
declares: sim; sweep; arena; leak
needs:
branch: jwal
resume: landed 09b8c12a 2026-09-29
created: 2026-09-29
---

## Why
The maintainer's first play session (INBOX 2026-09-29, value 5, front of the queue). On BF1 Kashyyyk (KAS2), the round wooden
platforms can be walked on top of, and their bamboo side walls stop a walking unit. But running at a side wall and
jumping forward carries the soldier INSIDE the hollow, under the deck. The maintainer: "players and AI getting places they
shouldn't will ruin the game."

## Evidence
- `lead_tmp/playtest_0929/kas2_platform_wall_outside.png` (the wall from outside) and `kas2_platform_inside.png` (inside:
  bamboo all round, the wooden deck as a ceiling). The models are probably `kas2_bldg_platform_*` (see ELFR's census).
- **The LEAK census reproduces it (2026-09-29, `game/examples/leak_census.rs`, on main before any fix).** No KAS2
  platform stands on the terrain: all 11 `kas2_bldg_platform_1` decks are at y 16.6-18.6 over terrain at about -5.9. The
  "hollow" is the HUT on the deck (`kas2_bldg_platform_roof`, body 386 at the platform (5, 16.65, -7)): a bamboo ring
  about 7.5 m in radius, whose floor inside is at 17.38, with the roof as its ceiling. From the deck outside, a jump at
  the ring ends inside it as a `push` leak: the wall pass pushes the landing soldier through (tick 20). Replay:
  `LEAK_NEAR=4.9,-7.06,14 LEAK_TRACE=1.28,4.66,jump target/release/examples/leak_census bf1_kas2` (stderr, the
  heading yaw -0.3077): JUMP at tick 18, and at tick 20, still rising (feet at 17.385), the wall pass pushes
  (0.254, 0, -0.799). That is 0.84 m INWARD, toward the ring's centre, and the axis crosses the wall face, whose normal is
  (-0.29, -0.30, 0.91). The eight sites round the ring are named in `leak_census_accepted.txt`. A fix should remove
  them, and the whole `push` class (1,675-10,131 runs per map) is this card's mechanism in general. The ring's collision
  also has four GAPS at the diagonals (`LEAK_SLICE=19.5`), which a walk passes straight through (ok class, e.g.
  start (7.16,16.70,-16.24) yaw 3.1200): check against the PS2 whether they are doorways.
- Suspects, to be proven or refuted, not assumed:
  - **Tunnelling at jump speed.** `CollisionWorld::wall_sweep` (`engine/crates/core/src/collision/walls.rs:331`,
    `SWEEP_CONTACT_DIST` 1e-3, `WALL_ITERATIONS` 2) casts per triangle. The header already records a "sweep says clear
    but the wall pass pushes" case. Check jump, sprint-jump and airborne steps against thin or one-sided wall triangles,
    and at a corner or seam where two collision bodies meet.
  - **Gaps in the baked collision** for that object: a slot at the top of the wall, at the seam under the deck, or a
    missing back face. Compare the port's baked collision (layer / `SoldierCollision` flags, `collision_layers.rs`)
    with the PS2's (`ps2-collision-layers-in-ram`: B1 `*(0x51568c)`).
  - **The PS2 may have the same hole.** Capture the spot in PCSX2 (RAM teleport, `ps2-camera-anywhere-by-ram-hold`)
    and try the same jump. If the PS2 leaks too, close it anyway (the maintainer's call) and record the deviation in the RE doc
    and FEEL_NOTES.

## Notes
- Pairs with LEAK (the 32-map census). LEAK is the instrument; this card is the known case and its fix. If LEAK is
  built first, KAS2 is its positive control.
- Walk-through decoration stays non-colliding (the maintainer 09-25); this is about solid walls.

## RE result (2026-09-29, lane 2, 905d179 — analysis/re-soldier-walls.md, [O])
- **The spot is City Hall, not the cp5 hut.** the maintainer's frames show a grass floor, the `kas2_bldg_platform_small`
  plank ceiling and the arched `kas1_wookiee_hut_walls`. That matches `kas1_bldg_hut_grand`'s four SOLID prim
  cylinders (bodies 103/106/107/109, r 9.25, y 13.0–17.0). LEAK's control (the cp5 `platform_roof` hut) is a
  different leak (M3, → WPSH).
- **M1, reproduced to the tick.** The repro starts at (130.146, 14.649, −121.827), yaw 90°, BF1
  imp_inf_storm_trooper, stick forward (+ L3), JUMP at tick 36:
  - the soldier is held at x 123.896 until tick 41;
  - at tick 42 the stepped capsule's rim contact normal is (0.82, 0.57), |n.y| ≥ 0.5, so it is skipped;
  - the ground probe (feet + 0.6) never reaches the top at 17.007;
  - he is inside at tick 43 and lands on the terrain inside at tick 61.

  It is general: a BF1 soldier walks through 0.5–0.75 m boxes and sheets (sprint 1.0) and jumps through ≤ 2.5 m.
  At City Hall's stairs a WALK leaks too, over a 0.63 m ledge into the front block.
- **PS2 (PCSX2, 8 jump trials at the lobes): no way in.** The wall zeroes the into-wall velocity and keeps the slide.
- **AI:** bots never jump. The authored route climbs the stairs' centre line; the hubs sit inside the lobes'
  footprint at terrain height. With the fix, bots also stop walking through 0.6–1.0 m prims (declare sweep).
- **Build = the header's resume (§7).** Split off: WPSH (M2 BF2 sprint-jump tunnelling + M3 inward push, the
  census's `push`), JARC (the PS2 jump rises 1.91 m against the port's 1.65 m) and SRAD (the PS2 0.72 m standoff).
  WTUN (the census's `tunnel`, suspect = M1) now waits on this card.
