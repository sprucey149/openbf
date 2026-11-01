# BF2 hero movement and animation fidelity

Work item: HERO_BEHAVIOR, 2026-11-01.
Baseline: `b5ca5cb`, including the existing pause picker.
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
6. Choke victim animation on BF1 maps / models: BF1 soldier assets lack authored
   human_choking clips because the original game had no playable heroes or Force
   powers. When heroes are enabled on BF1 maps, Animator::status_pose falls back
   to the baked lail clip (human_rifle_thrown_flail), giving victims active
   struggling and kicking animation in the hold instead of freezing motionless.
   In the simulation, victims snap to Stand and turn to face the caster (-direction),
   and running/prone targets are properly caught in the hold.

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

## Integration review follow-up

Restarting a custom action can reuse its current clip. A reset of the authored
combo clock now also restarts both layer players' blend clocks, so repeated
attacks blend into their opening frame instead of snapping there. The active
jump direction stays in the deterministic hash when the jump serial wraps to
zero. Ordinary soldiers with no directional jump retain their previous hash.

The `hero_behavior` capture example requires five arguments and a frame rate
between 1 and 1000; malformed invocations fail before loading or writing assets.

## Follow-up: hero combos, sustained Force powers and sprint (1930789)

The maintainer's six follow-up reports remain part of this hero-behavior work item:
Aayla's uneven saber combo, repeated held cast wind-ups, Force lightning resembling
an Arc Caster shot, a frozen choke victim, incorrect Force push reactions, and hero
sprint animation/movement/camera fidelity. The checkout was clean at
`193078918b7e` when this investigation began.

### Evidence boundaries

`engine/artifacts/hero-behavior/phase2/baseline-last.json` preserves the mutable
distribution snapshot before further runs. Its blackbox digest is beside it.
It is schema 13, **b5ca5cb90ce5+dirty**, compiled 2026-11-01 14:29:26 UTC,
checkout `193078918b7e`, BF2 `tat2`, seed 200, human unit 0. It is evidence from
the preceding executable, not an execution of clean 1930789. Recent frame times
are mostly steady (75 FPS, mean 13.3 ms, p99 15 ms, maximum 18.1 ms); initial
loading stalls do not explain the persistent posed-frame repetitions below.

The local PS2 disc closure and `SWBFRNT2.ELF-decomp.c` were checked against the
existing saber/layer/combo/jet/hero-flow research before extending it. Sprint
rate evidence in `re-sprint-clip.md` is existing audited work, not a new finding
from this session. Fresh PCSX2 evidence uses the maintainer's Mos Eisley Hero
Assault state 10: the preserved EE memory identifies Aayla's actor at 0x1cd3c10,
class 0x19b7170, animator 0x1bfc8a0, initially
(18.701675, -0.0199005, -82.365723). A PINE forward/sprint command intervention
recorded native mode-0 sprint updates. Input was intermittently overwritten by
the original controller update, so this is not an uninterrupted speed/rate
measurement and is not used to calibrate constants. A discarded AI actor-warp
attempt supplied no usable Force observation. State 10 was restored and the VM
paused; no save states or native gameplay constants were changed.

Fresh, matched PS2 held-lightning, push, choke and combo videos were **not**
completed. The executable/disc audit and port captures below establish concrete
defects and their corrections, not full visual parity with those original-game
actions. That comparison and the maintainer's windowed playtest remain open.

### Causes and corrections

| Report | Production cause and correction |
| --- | --- |
| Uneven main saber combos | The prior interpolation existed, but `Fx::advance_frame` called `SoldierVisuals::update` before applying the render blend. The windowed game therefore still sampled melee clocks at 30 Hz. It now applies the presentation pose before advancing visuals and reading effect attachments, then restores the simulation. The shared path covers all heroes, including authored full-body attacks; no blanket locomotion override or melee damage-clock change is made. |
| Held casts replay the whole animation | `CastClock` expired after the clip and a later Ability event started it again. Custom `Loop("FinalFrame")` metadata was collapsed into ordinary non-looping metadata. It now retains the authored last frame while that weapon is firing and held, then clears on release. The caster starts from the weapon's firing transition, including its wind-up, rather than waiting for the first emitted shot. Choke's existing held-channel behavior stays separate. |
| Force lightning looks like one Arc Caster shot | PS2 really does reference the same `com_sfx_lightning` asset, but Force authors intensity 0.15, five minimum bolts and angular spread 0.4. The port read spread from 0xce532e64 (an unrelated field at native class+0x10c); native parser 0x3156e0 maps **0xdd09f94e** to the spread at +0xcc. Five bolts consequently overlapped on one line. The correct property now fans the bolts out. First links follow the posed casting hand, and child-strand seeds refresh per display frame without consuming sim RNG. The Arc Caster path is unchanged. |
| Choke victim freezes | `Animator::status_pose` always sampled a non-looping clip, clamping to its last frame. It now repeats the authored BF2 `(frames-1)/30` period, and the victim's presentation age interpolates between ticks and restores exactly. Damage, target hold, lift and release gameplay are unchanged. |
| Force push distance/count/reactions | The emitter applied a flat normalized horizontal/radial shove through the map's knock rules. Native emitter 0x315f78 calls soldier vtable+0x90 (0x238b80) with inner radius `range*0.5`, outer radius `range*2`, and the authored push strength; 0x268fe0 subtracts half the scaled body radius for falloff, and 0x238b80 adds 0.6 up before BF2 knock dispatch. The positive Force push now uses that path's geometry, falloff and BF2 thrown/tumble selection on either disc's maps. Its authored cone, LOS, relations and four-target cap are retained and exercised with five candidates. Negative Force pull now replaces the target's momentum and scales from a point beyond it, through the same BF2 reaction. Ordinary blast push deviations and saber knock are unchanged. |
| Hero sprint feels wrong | The game ignored inherited `ControlSpeed sprint` rows. Jedi used 1.5 forward instead of their authored 2.5; Vader, Yoda and Grievous also have their own rows. Sprint forward/strafe factors now load by stance through the class chain, preserving the approved turn factor. BF2 sprint playback uses planar speed divided by the lower sprint clip's root speed, without locomotion easing or root-scale division (0x27caf0 case 0). Hero camera sections also load from their class chain: common Jedi stand is eye 2.0/back 5.2, sprint eye 1.8/back 4.0. The old infantry stand back 3.2 reversed the camera change. Infantry camera settings and the player's FOV option remain intact. |

All authored non-1.5 sprint rows in the retained class bake belong to Jedi/hero
classes. No hero health, weapon damage, unlock, offer, Force drain or target-count
balance values are changed.

### Port evidence and checks

`hero_behavior` now captures the production `Fx::advance_frame` path. Its explicit
`before-order` option reconstructs the old 1930789 animate-before-blend order
inside the current diagnostic executable; it is **not** footage from a separate
clean baseline binary. The comparable Aayla 75 FPS held-combo sequences show
40 identical successive posed meshes during active attacks in that control,
and zero after the shared clock-consumer correction. Their simulation hashes
match. Frame sequences, JSON traces and MP4s are under
`engine/artifacts/hero-behavior/phase2/`.

The Force captures use stationary clone dummies with 3000 HP and infinite caster
stamina on flat diagnostic ground, explicitly recorded in their headers. They
show held lightning, a continuously animated lifted choke victim, and four
thrown victims from five push candidates. They are pose/effect diagnostics,
not claims about normal-match damage or stamina balance. The selected map
supplies assets; the capture does not install that map's collision geometry.

Regression coverage includes the actual picker, assets, fixed-step sim and live
frame consumer for Anakin, Obi-Wan, Maul, Aayla and Dooku at 60/75/120 FPS, with
matching sim hashes. A gameplay-driven held-lightning case checks firing wind-up,
retained final-frame pose, and release. Existing real Bespin ceiling/infantry/jet
checks pass. Sprint rate checks cover every baked soldier/weapon set, and roster
tests exercise inherited hero movement/camera constants and ordinary infantry
defaults. Positive push tests cover lift, falloff, four-of-five targets and BF2
reactions on BF1/BF2 ground rules. Choke-loop and interpolated-age restoration
checks run through the animation and render-blend consumers.

Final validation for this follow-up:

- Core library: 1650 passed, 2 existing ignored.
- Game library: 665 passed, 8 existing ignored, including the added live firing
  transition/wind-up checks and 60/75/120 FPS determinism checks.
- Roster integration group: 42 passed, including the inherited hero sprint and
  camera checks. The extended melee/choke render-clock restoration check passes.
- `cargo fmt --all --check` and `git diff --check` pass. Clippy for core/game and
  all targets passes with `-D warnings -A clippy::let-and-return`; that exception
  is the existing `force_can_target` return-local lint, not a newly allowed lint.
- Release `game` and `hero_behavior` were rebuilt. The stored assets were not
  rebaked for this check; a new bake also lends `human_choking` to BF1 soldiers
  through `BORROWED_BF2_STATUS`. A pack of the existing `engine/assets_baked` and
  `engine/missions` loaded embedded BF2 Mos Eisley with `--map assets_baked/tat2
  --headless --scene-report`: 2708 draws, 727 instances, zero untextured or
  dangling draws. The pack held 24,574 files and was 2,908,633,842 bytes.

The clips, traces and pack logs of this investigation stayed on the contributor's
machine and are not in the repository.

The requested maintainer playtest and full matched PS2 visual comparisons remain
the fidelity acceptance gap; passing these checks is not a claim of final parity.
