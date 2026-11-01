# BF2 hero movement and animation fidelity

Work item: HERO_BEHAVIOR. Owner: maintainer-directed Codex session, 2026-11-01.
Branch: `codex/bf2-hero-behavior`, based on `b5ca5cb` with its existing pause picker.
Status: implemented; original-game visual acceptance remains open.

Scope: moving saber attacks; moving block and its exit; solid-ceiling containment
during repeated double jumps; directional jump/flip selection; persistent animation
timing and transition jitter. Health, damage, offers, unlocks and Force balance are
outside this work item. Existing human look freedom and Aayla presentation defaults
remain approved deviations.

## Preserved baseline

`engine/artifacts/hero-behavior/baseline-last.json` was copied before any game runs.
SHA256: `a337e52326768d99117f75a1c8d521edd1da72d43bde4e6961f750a45d204d84`.
The `blackbox_read` digest is retained beside it. Schema 13, build
`b5ca5cb90ce5`, built 2026-11-01 13:33:49 UTC; checkout agrees; map `bf1_bes2`,
seed 198, human unit 0, `rep_hero_anakin`. The digest's player-team display is
one-based. The capture establishes movement/jump inputs, not animation-layer or
ceiling causes. The warm frames are mostly steady at 75 FPS: p50 13.4 ms,
p99 14.4 ms. Class loading caused 1,157 ms CPU and 97 ms GPU/resource hitches;
these are separate from persistent pose timing.

## Investigation evidence

Existing references read: `re-bf2-saber.md`, `re-soldier-anim-layers.md`,
`re-jetpack.md`, `re-bf2-hero-flow.md`, and `INDEX.md`.

Fresh source observations, not fresh original-game visual validation:

- `Animator::combo_pose` samples a complete lower-or-upper pose and overwrites
  locomotion every display frame, without custom-layer eligibility or blending.
- `SoldierVisuals::step_on_foot` supplies the fixed-tick `MeleeState::pose` clock
  directly. Ordinary animation advances per display frame; saber poses do not.
- `push_out_soldier_from` subdivides only XZ displacement; its contact response
  returns exactly zero Y. Flat ceilings therefore cannot arrest upward motion.
- `Soldier::jet_on` implements the double-jump impulse but omits native
  `FUN_0027b9e0` (local BF2 decompile lines 199444 onward). The native selector
  uses the current forward/artist-strafe pair at the second-jump request,
  squared magnitude threshold 0.0625 and dominant-axis buckets. This is distinct
  from initial takeoff. Same-direction backward/side requests can refuse an
  early restart; forward/stationary requests restart.
- Native `FUN_0027dfe0` (201133 onward) composes custom upper and lower slots.
  Unless forced, lower eligibility is released when actual local movement
  differs from the lower clip's packed root velocity by more than 0.3 m/s.
  `FUN_0027c978` receives the forced flag from `FUN_0033c4b8`, state bit 8.

Local PCSX2 2.8.2, BIOS, original BF2 disc and save states are available.
The disc boots and existing states load through PINE. Fresh controlled hero
visual comparisons were not obtained: keyboard taps did not reliably operate
the game, and the replay file dialog's automation focus/accessibility state did
not agree with its displayed filename field. No original-game visual parity is
claimed. Temporary keyboard bindings were restored byte-for-byte; existing save
states and memory cards were preserved. The attempted generated input movie was
not accepted as evidence. Its format was checked against the official PCSX2
2.8 source under `pcsx2/Recording/`; no emulator code entered the engine.

## Cause and correction

1. Moving attacks: the former unconditional full-pose overwrite bypassed the
   two players. Combo upper/lower clips now enter those production players with
   their masks and native custom descriptors. Lower eligibility is latched off
   when local motion differs from the authored lower root velocity by >0.3 m/s.
   Standing attacks can use their legs. Active `AnimatedMove` retains its forced
   lower pose and unmodified deterministic desired-motion integration.
2. Block: change-weapon intentionally maps to BF2's toggle block. That mapping
   remains. The same lower eligibility rule keeps locomotion alive in block;
   normal slot selection and blend-out remove the custom pose on exit. Deflect
   impact poses retain their separate clocks and existing energy/contact rules.
3. Ceilings: a new SOLDIER-layer capsule cast tests the rising 3D displacement,
   including mesh triangles, before the existing wall/floor passes. It clips
   at first ceiling contact and removes into-surface displacement/velocity;
   tangential sliding survives. Floors and wall stepping keep their own passes.
4. Directional jumps: initial takeoff retains `jump`. A successful Jedi second
   jump captures the then-current stick, not later air steering. It selects
   stationary/forward/backward/left/right with the native dominant-axis rule.
   Back/side repeat-restart gating and the 0.5 s posture gate are preserved.
   Rendering restarts the selected simple clip on an accepted request. Missing
   authored forward variants retain the existing BF2 bank fallback to `jump`.
5. Uneven animation: the render blend now interpolates the selected melee clock
   between fixed ticks, restores it before simulation resumes, and handles new
   clips/restarts without interpolating unrelated clocks. Custom clips use the
   ordinary masked blend players, authored blend-in/out and `BlendTimeTo`, and
   loop/final-frame metadata. Their blend clocks advance in display seconds,
   independently of locomotion playback rate. Damage, blade contacts and combo
   transitions continue to read the fixed-step clocks.

## Production evidence and reproducibility

Artifacts stay local under `engine/artifacts/hero-behavior/`.

- `before_anakin75_valid/`: clean `b5ca5cb` gameplay/animation modules, selected
  using the existing `ClassPicker::select`. `baseline-capture-valid.exe` retains
  the capture executable. Flat diagnostic ground and actor reset per action are
  explicit interventions. The earlier folders without `_valid` contain a
  rejected capture setup with no body draws and are not evidence.
- `hero_behavior` renders real skinned bodies through `SoldierVisuals`, the
  render blend and the renderer. Its cases cover standing, forward, backward,
  lateral attacks, block entry/movement/exit, and all five jump directions.
  Anakin is captured at 75 Hz on a BF1 map; Obi-Wan at 60 Hz and double-bladed
  Maul at 120 Hz on a BF2 map. These are controlled display-delta captures,
  not measured windowed frame-rate benchmarks. `frames.json` retains inputs'
  action labels, pose clocks and mesh movement. New captures stamp build/dirty.
- `anakin-before-after.mp4` and `anakin-before-after-valid.png` compare the same
  action sequence, before on the left and after on the right. The neutral
  background isolates the body. Frame sequences remain available per action.
- `ceiling-before.json`: the production movement path with only the new ceiling
  response disabled. New visual jump fields remain present; stationary inputs
  make no physical difference from the old path. No legacy renderer is inferred
  from this experiment. At (-241,-33.1,186.6), local tick 13 / global tick 14
  crosses the underside: feet Y -31.126419 -> -30.588938, upward velocity
  16.124437 m/s. Body 378 is instance 136, `bes2_bldg_square_bot2`. The segment
  first meets the capsule ceiling at fraction 0.344810 with normal about -Y;
  the old horizontal wall response cannot arrest it. Repeated jumps escape to
  Y 106.63. The neighboring x=-242/-240 probes reproduce the same body.
- `ceiling-map.json` is the restored production response. The three covered
  probes remain below that underside during 300 ticks of repeated jumps.
  (-224,187) is an open-sky control; (-225,190) contacts a street lamp, not the
  platform ceiling. Rising in those controls is not classified as a slab leak.

The captured baseline snapshot and the reproduction are different evidence:
the snapshot belongs to clean `b5ca5cb90ce5`; fixed builds belong to
`b5ca5cb90ce5+dirty`. No fix is attributed to the original snapshot.

## Checks and remaining gaps

Focused tests exercise actual hero classes through the picker, fixed-step sim,
render blend and soldier visuals at 60/75/120 Hz, check all five jump selections,
and require identical simulation hashes for matched fixed-tick input streams.
Synthetic production `Sim::step` tests cover repeated jumps under slab, thin
mesh and sloping ceilings; a real Bespin test covers Anakin, normal clones and
jet troopers at three underside positions. Existing infantry, jet, aim,
authored movement, blade/contact and hero-default tests remain in the suites.

The strict Clippy run encounters a pre-existing `clippy::let_and_return` in
`core/src/sim/force.rs:69`. A second run allows only that existing lint and
otherwise keeps `-D warnings`; it passes. No unrelated Force code is changed.

Remaining fidelity gap: fresh matched original-game hero clips, live directional
restart observations, and a human windowed playtest are still required for visual
parity acceptance. Existing source/disc research supplies the behavior rules;
the new port frame sequences establish the corrected composition and clocks,
not independent PS2 equivalence. No first-use asset-loading optimization is
claimed. The retained class-picker loading stalls are separate.

Distribution refresh and final check results are recorded below after completion.
