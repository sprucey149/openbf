---
id: UORDS
title: Bolts are swept every tick against units, as on the PS2: the predictive spawn ray tests only terrain and buildings (BF1 0x48 / BF2 0x90), soldiers / vehicles / riders / water / foliage / shields are struck by a prev->current sweep each frame (0x996 / 0x132e) - a soldier who steps out of a bolt's line is missed, one who steps in is hit (the port's split landed in main's 0ee5a57; its PS2 evidence is open)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
analysis/re-ordnance-kinds.md §2 (card UORD, [O], two verifiers): every class of the bolt family (bolt, laser, bullet; also missile and shell) shares one flight core and one per-frame sweep. B1 FUN_002764c0 casts ONE predictive ray at the first update with mask 0x48 (buildings + terrain only) when class+0x88 = 1; the per-frame sweep FUN_00273128 (called by FUN_00163198 for every moving ordnance) rays prev -> current with 0x9de until that ray ran, 0x996 after: soldiers 2, vehicles 4, riders 0x10, water 0x80, foliage 0x100, shields 0x800. BF2 FUN_00313600 / FUN_0030fad0: 0x90, 0x13be / 0x132e. This corrects re-blaster-damage §5.1 ('targets cannot dodge'), which the port's core/src/projectile.rs transcribed when this card was written; its then-test `predictive_raycast_hits_dodgers_and_ignores_late_entrants` (then at `sim/combat.rs:739`) asserted the opposite of the PS2. Main's 0ee5a57 replaced that path on 2026-10-27 - see What is in the tree.

## Evidence
- The doc §2.2 (lines B1:197629-197656, B1:195510-195514, B1:2731-2738; B2:304258-304304, B2:301923-301925); verifier batch 1 C3/C4 SUPPORTED, T2 SUPPORTED by both absence readers.
- Foliage contact (B1 layer 0x100 / B2 0x200) returns 0 in the base collision (B1:195309-195310): the bolt flies on.
- Life-out spawns ExplosionExpire (class+0x34 / +0x48) for every class of the family (B1:195365-195367): the port spawns none for a bolt; [H] a bolt whose predictive ray hit terrain may fire BOTH ExplosionExpire and the impact explosion (BF1 jet trooper rocket authors both) - check on the PS2 before building that part.

## Acceptance
- Scene tests, mutation-checked: a soldier who steps out of the line after the shot is missed; one who steps in is hit; a wall / terrain hit still ends the bolt at the predicted point; foliage does not stop a bolt; water does.
- A PS2 check of one dodge or one late entrant (lead_tmp/uord/ps2/missile_b1.py style sampler on a laser instance, vtable 0x53a568) printed beside the port's.
- 8 seeds x 32 maps 256 / 256; kills a minute and team kills per map within the seed noise, or the move named for the planner (it is a change to every firefight).

## Notes
- Missiles and shells already need the full-mask per-tick sweep (card UORD): build the shared sweep once; whichever card lands second reuses it.
- 2026-10-30: the unit / hull sweep is `Sim::sweep_bolts`, which every round already uses; `Sim::probe_flight` (this branch) is the world chord that only falling and steering rounds need per tick.
## Note (AUDIT 2026-10-22, lane 2): the PS2 side of the acceptance check exists
- `analysis/re-ordnance-kinds.md` 2.4 [M], both games: a bolt's line written through a stationary soldier's chest
  after its predictive ray ran stops one update later at the chest (BF1 15 / 15, 0.35-0.59 m short of the chest
  centre; BF2 15 / 15, 0.17-0.50 m) and hurts him (14 / 15, 15 / 15); the line 2.5 m aside flies on (14 / 15, 13 /
  15). In battle 0 of 661 BF1 / 409 BF2 predictive kept hits belong to a soldier. Probes and rows:
  `engine/artifacts/lead_tmp/audit-uord/ps2/` (`bolt_b1.py`, `bolt_b2.py`, `knock_read.py`, `b1_knock2.json`,
  `b2_knock1.json`, `knock2_table.txt`, `knockb2_table.txt`). The port's scene test can mirror this knock-in (late
  entrant hit, 2.5 m aside missed, moving target dodges) and print the PS2 table beside its own.

## What is in the tree (2026-10-30): the split came from main, not this branch
The behaviour the card asks for is in the ordinary fire path, added by main's 0ee5a57 ("Make hits match what you
see, and add headshots and vehicle weak spots", 2026-10-27), an ancestor of this branch's 70da755. This branch adds
nothing to the bolt path; its shared piece is the shells' / missiles' world probe.

- **The spawn ray tests terrain, water and static bodies only.** `Bolt::fire`
  (`engine/crates/core/src/projectile.rs:228-265`) calls `spawn_past` with `std::iter::empty()` (`l.260`) and
  `HullFilter::Static` (`l.258`): the terrain crossing (`l.336-345`), the water crossing (`l.346-351`) and the map's
  static bodies (`l.355-363`) are tested; the unit-capsule loop (`l.364-371`) is reachable only through `Bolt::spawn`
  (`l.299-314`), which no shot uses - its one runtime caller is the bot's own-blast query
  (`ai/gunner_foot.rs:713-744`), the rest are tests. So no soldier is a predictive-ray candidate.
- **The per-tick sweep.** `sweep_bolts` (`sim/combat.rs:41-199`) takes each round's prev->current segment
  (`from` / `to`, `l.79-80`) and tests every alive unit's `BoltPrims` (`l.102-110`, with the human meet lag and the
  headshot multiplier), droideka bodies and shields (`l.112-140`), mines, droids and turrets (`l.141-164`), guided
  rockets (`l.165-182`) and the vehicle hulls (`l.183-191`); the nearest writes `hit`, `end` and `remaining` (`l.192-197`).
  It runs at step 5 after `fly_missiles` and `fly_shells` (`sim/step.rs:980-982`), so shells and missiles get it too.
- **The outcome is tested.** `a_bolt_hits_what_it_passes_through` (`sim/combat.rs:1244-1272`) teleports the target
  10 m aside after the shot and asserts it is missed (`l.1254`), then moves a unit into an unaimed bolt's path and
  asserts it is hit (`l.1264-1271`). `terrain_blocks_bolts` (`l.1276`), `water_stops_bolts_and_names_the_surface`
  (`l.1306`) and `world_model_blocks_bolts` (`l.1379`) cover the spawn ray's stops.
- **This branch's piece.** `Sim::probe_flight` (`sim/missiles.rs:20-53`) is the per-tick world chord for a shell or
  a missile: it calls `Bolt::fire` with this tick's step (`l.38-48`) and copies the hit, end and remaining
  (`l.49-52`); `fly_shells` (`sim/ordnance.rs:128-138`) and `fly_missiles` (`sim/missiles.rs:70-93`) run it at
  step 5 before `sweep_bolts`. A bolt keeps its one spawn probe plus the sweep.

## What is open (the acceptance, not the build)
- **The PS2 dodge / late-entrant comparison** (the card's second acceptance item): not run. The probes and rows
  are absent (below), so the port's table cannot be printed beside the PS2's.
- **The mutation check** on the scene tests: not run. `lead_tmp/audit-uord/mut/M1` is false.
- **The 8 x 32 sweep 256 / 256**: not run; the run is missing.
- **Foliage**: the port draws foliage as render-only model instances (`engine/crates/game/src/viewer/foliage.rs`),
  not as collision bodies; `Layers` has no foliage bit (`collision/shape.rs:429-439`), so a bolt cannot meet one
  and the acceptance case cannot be exercised. The code path is "passes" by construction, not by a test.
- **A shared `probe_flight` for bolts**: not done and the behaviour does not depend on it. The card's Notes asked
  for the sweep to be built once; that is `sweep_bolts` for units, hulls and the rest, which every round already
  uses, while `probe_flight` is the world chord only the falling / steering rounds need per tick.

## Evidence (2026-10-30)
- Source: `projectile.rs` and `sim/combat.rs` are at HEAD (70da755), not modified by this branch; the bolt path
  above is main's 0ee5a57. Run from `engine/`: `cargo test -p core --lib a_bolt_hits_what_it_passes_through` ->
  1 passed, 0 failed (1,410 filtered).
- **The PS2 bolt captures are ABSENT.** `Test-Path` false for
  `engine/artifacts/lead_tmp/audit-uord/ps2/bolt_b1.py`, `bolt_b2.py`, `b1_knock2.json`, `knock2_table.txt`, and
  for `lead_tmp/uord/ps2/missile_b1.py`; `engine/artifacts/lead_tmp/` has no `audit-uord` directory. The doc's
  table survives (`analysis/re-ordnance-kinds.md:104-125`: 15 / 15, 14 / 15, 15 / 15, 13 / 15, 0 of 661, 0 of
  409); the probes behind it do not.
- Acceptance, item by item:
  | item | status |
  |---|---|
  | a soldier who steps out is missed | MET - `sim/combat.rs:1244-1254` |
  | a soldier who steps in is hit | MET - `sim/combat.rs:1256-1271` |
  | a wall / terrain hit ends the bolt | MET - `terrain_blocks_bolts` (`sim/combat.rs:1276`) asserts the hit; the end point is `origin + dir * best_t` (`projectile.rs:386`) |
  | water stops a bolt | MET - `water_stops_bolts_and_names_the_surface`, `sim/combat.rs:1306` |
  | foliage does not stop a bolt | NOT TESTABLE - foliage is render-only |
  | mutation check | NOT RUN |
  | PS2 dodge / late entrant printed beside the port | NOT RUN - captures absent |
  | 8 x 32 sweep 256 / 256 | NOT RUN |
- Status stays `blocked` for the missing captures and the unrun checks, not for missing bolt code.
