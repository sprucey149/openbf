# BF1 Geonosis Hailfire weapons and wheel motion

2026-11-14. Maintainer-requested continuation on `feature/vehicle-parity`,
source base `a4ad72f9ea3935f453cb141ce5a557abcd5e6b62` plus the retained
unfinished suspension changes. No issue was opened or public ownership changed.

The maintainer chose **BF1 guns, fixed**, and clarified that missing wheel
animation, rather than camera placement, caused the visual problem. The camera
law and class camera values are unchanged.

## Result

- BF1's R1 launcher retains five rounds, 0.25 s shot delay, 2.5 s reload,
  TurnRate 8 and its authored waver. The previous BF2 TurnRate 0 combined
  with BF1 waver let missiles wander without steering back toward their aim.
- L1 retains BF1's spider cannon: 0.225 s shot delay and native heat. Its
  isolated gun, ordnance and explosion avoid the Spider's STAP normalization.
  Person/shield damage uses the existing BF1 x4/3 scale conversion. Native
  vehicle damage and the normalized 8,000-HP hull remain. BF2 guns are untouched.
- BF1's two hardcoded wheel scroll rates now feed the existing independent
  vehicle texture instances. Forward and reverse move both wheels; yaw gives
  opposite wheel motion. BF2 keeps its authored wheel properties.
- Both games' Hailfire centre and suspension arms use the class nodes and the
  deterministic, hashed spring state. World and vehicle contacts supply the
  suspension kick. See `re-hover.md` 6.20 for original function/line evidence.

## Evidence and checks

The latest F9 black box was read first: `engine/artifacts/playtest/last.json`,
BF2 Geonosis, exit tick 251, clean build `a4ad72f9ea39`. It is not a BF1 bug
capture, so no BF1 behavior was inferred from it.

The maintainer's BF1 slot 5 was read without changing it or converting it.
SHA-256: `e64b0b2c0615576665509d827bbdd29f8ba72a8374f4a3b44b88548aaaba2bf2`.
Both original Hailfires have suspension displacement -0.125 m. The loaded
port battle rig agrees with their centre exactly; the maximum arm matrix
entry error is 2.3841858e-7. Private fixture and captures remain under
`engine/artifacts/hailfire-bf1/`, outside Git.

Commands (repository root, Windows/MSVC, local owned bakes):

- `cargo test --manifest-path engine/Cargo.toml -p core --lib vehicle::hover`:
  30 passed, including rest/kick/settle and suspension hashing controls.
- `scripts/gpu_tests.sh --game-lib vehicle_class::tread`: 4 passed, including
  BF1 forward/turn/still controls and the unchanged BF2 authored rates.
- `scripts/gpu_tests.sh --game-lib suspension`: 1 passed, checking the
  synthetic centre/arm/descendant pose.
- `HAILFIRE_NATIVE_NODES=engine/artifacts/hailfire-bf1/native.json cargo test
  --manifest-path engine/Cargo.toml -p game --test suite hailfire --
  --nocapture --test-threads=1`: native pose, launcher clip/reload and existing
  BF2 gatling/audio controls passed. The native JSON argument is optional;
  when supplied it must exist and all three matrices must match.
- `scripts/gpu_tests.sh --game
  bf1_hailfire_draws_scrolling_wheels_and_moving_suspension`: 1 passed,
  headless and serial. Scroll changed 5,533 pixels, compression 4,814;
  duplicate draws at one tick are byte-identical. All three PNGs were viewed.
- `cargo test --manifest-path engine/Cargo.toml -p game --test suite
  normalization -- --test-threads=1`: 20 passed, 1 table-printer ignored.
  The existing explosion-copy assertion now distinguishes class identity from
  the visual properties it checks; the copied/original class names are also pinned.
- `python scripts/check_provisional.py`: no new values without measurement plans.
- `git diff --check`: passed.

The full core suite ran 2,353 tests: 2,346 passed, 3 ignored, 4 failed in
unchanged tests: `cached_bounds_cost_is_bit_identical_to_parry`,
`max_and_min_are_x86s_std_answers`,
`no_std_transcendentals_in_game_sim_inputs`, and
`a_leg_that_is_not_finite_is_answered_by_every_barrier`. Three concern signed
zero; the fourth rejects the existing minimap test-module layout. These were
not changed as part of the Hailfire task; a baseline rerun is not recorded.

The source map was refreshed for touched files and the previously unlisted
wheel modules. Its checker needs Python and slash-normalized paths on Windows;
run from its embedded Python with those host adaptations, it still reports
unrelated missing/stale entries. No source modules were added by this change.
Workspace-wide `cargo fmt --check` reports existing formatting differences;
changed Rust files were formatted directly with `skip_children=true`.

## Unchecked acceptance

Dynamic PS2 collision/suspension traces remain unchecked. The maintainer
confirmed the fixes in play after the secondary terrain-impact correction
(2026-11-14: "great, it's all fixed"). The slot comparison establishes the retained resting pose, not a
full moving-frame comparison. No new round sweep, per-map bot population or
simulation timing measurement was made. Full alignment acceptance was not run.


## Follow-up: right-click cannon hits beneath the vehicle

The maintainer's next playtest reported this with a screenshot. The new
`engine/artifacts/playtest/last.json` is BF1 Geonosis, exit tick 583, build
`a4ad72f9ea39` plus dirty diff `e200d8e8c20e7a41`. Human 0 is mounted in
vehicle 10 at (194.37, -63.86, -29.81). Its secondary shots start at the
vehicle root; at that point the terrain is -63.63915, about 0.22 m above
the muzzle. Re-settling near that pose retains a below-sheet root.

This root origin is original behavior, not a missing barrel to invent:
BF1 slot 5's secondary aimer 0x1d17300 stores the entity root as its two
world fire points (+0x80/+0x90); its local point is zero. The native root
is (139.37044, -62.29406, -22.70134); the matching baked terrain sample
is -62.14662. A headless native replay of slot 5 with L1 held 20 emulator
frames fires ahead at the reticle instead of exploding beneath the vehicle.
The native screenshot is retained privately as
`engine/artifacts/hailfire-bf1/native-secondary.png`. The original save
was read only; a separately converted copy was loaded using the pcsx2-state
skill. Tracking run `01M7E23JZ0B1RYM8VB00247TB4` is finished and its isolated
emulator generation was stopped.

The port's predictive bolt ray incorrectly counted either direction through
the terrain sheet as a hit. The outward crossing becomes a near explosion
when the muzzle starts under the ground. BF2 FUN_0055f940,
`SWBFRNT2.ELF-decomp.c:683274` onward, bisects only a downward piece after
having been above the terrain (`re-grenades.md` 13.2). The native BF1 check
and `re-aim-assist.md` 10.5's below-ground observations agree with that
one-sided behavior. The port now supplies a front-side terrain ray to bolt
spawn/prediction, retaining its existing crossing precision. The camera and
other two-sided sheet consumers keep their existing ray. World obstacles,
cutters, hull filters and weapon origins are unchanged.

The reported-pose regression compares the old underside crossing with the
corrected bolt path and checks any later terrain impact against the existing
native terrain-query implementation. A synthetic below-sheet/upward ray must
escape; a ray that emerges and then reaches another hill must still hit that
hill; above-ground shots and the camera retain their former answer.


Follow-up verification: `cargo test --manifest-path engine/Cargo.toml -p core
--lib -- --quiet` ran 2,354 tests: 2,347 passed, 3 ignored and the same four
unchanged failures listed above. The new `bolt_terrain_ray_passes_the_underside_but_still_hits_the_next_hill`
regression passed. Output is retained privately in
`engine/artifacts/hailfire-bf1/secondary-core-tests.txt`.

The BF1 playtest-position regression passes at level look and the recorded
-0.07-radian pitch. Old underside distances are 0.750 / 0.875 m; corrected
terrain impacts are 62.750 / 55.250 m. Both later impacts agree with the
native terrain-query implementation within the 0.3 m marched-ray tolerance.
The actual firing origin remains at the vehicle root. The final Hailfire suite
retains the slot-5 pose and BF2 audio/launcher controls.
