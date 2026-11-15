# AT-AT head guns and footsteps, 2026-11-13

Scope requested by the maintainer on `feature/vehicle-parity`: BF2 Hoth's
alternating gun origins, absent mounted muzzle flash, audible AT-AT and AT-ST walking.
Source revision before edits: `f14ee6d9`. No issue created. Existing save-state
tool changes in the working tree belong to separate work.

The F9 snapshot `engine/artifacts/playtest/last.json` was absent. The two
maintainer screenshots show the PS2 view and the port's right-side flash.
The maintainer's slot 8 (`SLUS-21240 (02F4B541).08.p2s`) was inspected and
imported to ignored `engine/artifacts/atat-slot8.p2s`. Only the version stamp
and controller numbering changed. A headless PCSX2 fd9d310 session loaded it
successfully on port 47802; local frame captures are under `engine/artifacts/atat-*`.
L1 firing captures at 6 and 14 advanced frames show no near-head muzzle flash.
These images establish appearance, not an audio or volley-timing measurement.

## Cause and implementation

The head weapon's barrel slots name `hp_fire_Lgun` and `hp_fire_Rgun` as fire
points. The chin weapon instead uses the `barrel_Lcannon` and `barrel_Rcannon`
aimer nodes without explicit `FirePointName` writes. `AimerSpec::muzzles`
includes those fallback nodes, while the separate `fire_points` list does not.
The simulation builds its ring from `muzzles`; the drawn chin shot instead read
`fire_points`, falling back to the seat's last fire point (`hp_fire_Rgun`).
`walker_shots::frame` now uses the same firing aimers and barrel-slot selection
as the simulation, including rejection of turn-only aimers.
This follows the fire-ring evidence in `re-blaster-damage.md` §3.1
(BF2 FUN_00301d28; aimer setter FUN_00302f10).

The Hoth regression also exposed the parser assigning the seat-level
`aimer_head`, authored before any `WeaponName`, to weapon zero. It now retains
that aimer for head steering with no weapon-ring owner, so it adds no phantom
third muzzle. This changes the head weapon's simulation ring from three points
to its two actual barrels; no damage or cadence numbers change.

Mounted guns inherited infantry flash settings and reached `flash_draws_at`.
They now skip that draw. PS2's weapon render callers exclude mounted guns:
`re-muzzle-flash.md` §5, BF2 FUN_00239bb0 lines 155363–155377 and
FUN_0032b0e0's single caller. This is a renderer correction, not an AT-AT
weapon-data override.

Walking had no footstep voice producer. The new `engine_sound::walker`
reads the ordered foot primitives and `FootstepSound<i>`, poses each primitive's
underside with its parent-bone palette and the hull rotation, and applies the
per-update stomp detector. The AT-AT uses four cylinder feet and detection type
1: emit on the unlatched `previous-height - height < 0.1` transition; rearm when
the descent exceeds 0.1 in one update. Idle clears latches. Voices originate at
the foot's horizontal position and hull height and use the existing one-shot
mixer, sound layers, attenuation and black-box sound logging. Destroyed or
replaced vehicle generations discard detector state. Steps run independently
of whether an engine-loop record resolves.

Evidence: `re-vehicle-sounds.md` §7, BF2 FUN_0025aaa0 lines 176135–176157
(detectors) and 176237 (step sound); BF1 FUN_00216b30 lines 128139–128158
and 128224. Hydraulics, stomp effects and rumble remain outside this fix.

## Validation

`scripts/gpu_tests.sh --game --include-ignored walker_shots`, headless and serial:
9 passed, 0 failed, 547 filtered out. AT-AT recorded 18 head shots and
5 chin shots in 150 ticks per trigger, each matched to the independently named
animated left/right nodes. Both inherited active flash settings but produced
no flash draw using the GPU-loaded muzzle model. Four authored foot definitions
resolved in the Hoth sound bank and produced 7 step voices in 400 walking ticks;
idle and destruction produced none. Existing AT-TE and tracer regressions pass.
The AT-ST fixture instantiates Hoth's installed snow AT-ST pad specification
independently of pad availability, then samples both authored walking half-cycles
(animation entries 7 and 8) for 180 ticks. It produces 9 step voices, using
`com_weap_layered_at_step`, with origins on both sides of the hull.

`cargo test --manifest-path engine/Cargo.toml -p game --test suite vehicle_class -- --test-threads=1`:
15 passed, 0 failed, 541 filtered out.

`cargo test --manifest-path engine/Cargo.toml -p game --lib engine_sound -- --test-threads=1`:
18 passed, 1 ignored (owned Space Felucia maneuver fixture), 994 filtered out.

`cargo build --manifest-path engine/Cargo.toml --release -p game`: passed;
`engine/target/release/game.exe` rebuilt from the changed production sources.
Scoped `git diff --check` passes. No rebake is needed for these existing manifests.
Local verification logs: `engine/artifacts/walker-parity-final.log`,
`atat-class-tests.log`, `atat-engine-tests.log`, `atat-release-build.log`.

Unchecked: listening comparison of the final port mix against slot 8, complete
GPU/alignment acceptance, original-game volley timing, and other walker classes
beyond the AT-AT/AT-ST checks. Per-map 60 fps bot population and simulation
timing remain unmeasured for this change.
