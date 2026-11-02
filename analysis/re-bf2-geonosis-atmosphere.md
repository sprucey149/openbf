# BF2 Geonosis atmosphere

[O] UNAUDITED: source reads and local measurements below have not received an
independent re-derivation.

Implemented on `map-atmosphere`, updated through upstream `9a7d520` (original implementation baseline `0969996`).
Original assets and captures remain local and ignored. This work covers BF2
SkyObject flights, dome-attached battle effects, FogCloud, and authored sky glow. It does not close
BF1 SKYO, the separate DustEffect class in DUSTE, or GODR.

## Upstream integration (2026-11-02)

Fetched and fast-forwarded to origin/main 9a7d520, then reapplied the complete
feature work. The pre-update working tree and index patches are saved locally
under `engine/artifacts/geo_re/pre_origin_merge_*.patch`; the autostash is retained
as an additional backup. Four textual conflicts were resolved: generated analysis
index, bake output construction, Manifest definition and its wire-order test.
Both the new upstream terrain_query_size and this feature's fog_cloud are kept.
The index was regenerated using the updated script.

Upstream's render culling/sort optimization and direct palette-buffer upload
were retained alongside the fleet/background blend ordering. Its gameplay flyer
changes remain separate from decorative flight; no core/input/vehicle code is
changed by this feature relative to the new main. Donor render recovery and
alpha-aware sky texture tier selection remain intact. Post-merge checks and
captures are recorded separately under `post_origin_*` to avoid presenting
pre-merge results as validation of the combined build.

The combined build passes workspace all-target compilation, 344 renderer/schema/
parser unit tests, 78 FX tests, twelve sky GPU tests, ten skinned GPU tests and
the real-disc sky-mask bake test. All 201 Geonosis frames, 21 Hoth frames and
three isolated fleet close-ups are byte-identical to the pre-update build.
Pause redraws remain identical and the 100-tick simulation hash remains
5573f2b603ed95f4. Receipts and logs are under `post_origin_*`.

## Reference and coordination

Read PLAN, SKYO, DUSTE, FOGN and GODR before implementation; checked SUNG before extending the sky material path. The public repository
has no open issues or contributor branches at the check on 2026-11-02. Private
loop STATUS/INBOX files are absent from this checkout. Updated the feature branch
with `git pull --ff-only --autostash origin main`; preserved the local shading
and pause fixes made by Gemini. No issue or PR comments were posted.

Reference: `extracted/bf2/lvl/DATA/_LVL_PS2/GEO/GEO1.LVL`, the BF2 USA v2.01 ELF
extracted into `engine/artifacts/geo_re/SWBFRNT2.ELF`, and the local original game
in PCSX2. The existing slot 10 save was read without overwriting it. RAM and
VIF packet receipts are under `engine/artifacts/geo_re/reference_state/` and
`original_cloud_check.json`. Temporary original-game cloud diagnostics were
restored: enable 1, density 100; the emulator was left paused after that reference capture.

## Ships: authored flight, rather than a camera-following ring

The disc specifies two `rep_fly_assault_DOME`, twenty `rep_fly_gunship_DOME`,
twenty `cis_fly_droidfighter_DOME`, and eight `cis_fly_techounion_DOME`.
The loader at 0x3beba0 stores NumObjects at 0x3bed80 and allocates count x 112
bytes at 0x3bee58. The old bake's cap of 16 was losing four of each small craft.
The original live counts are **2, 20, 20, 0**. The Techno Union geometry fails
to resolve in the original too; inventing eight replacement ships would be wrong.

| Craft | Height | Distance | Local velocity | Direction factor |
|---|---|---|---|---|
| Assault ship | 200..300 | 1500 | Z 20..50 | 2 |
| Gunship | 80..140 | 600 | Y -10..10, Z 80..120 | 0.5 |
| Droid fighter | 80..140 | 300 | Y -10..10, Z 80..100 | 0.5 |

Range parser 0x3beb70 stores the first endpoint and the difference. Spawn
0x3bf270 normalizes an XZ square sample, picks height and local velocity ranges,
builds a tangent with a random side and inward bias, and initializes age to
lifetime x (0.8 + 0.2 x random). Update 0x3bf148 integrates acceleration in the
local frame and then world position; basis remains fixed. The 0x3bef30 wrapper
recycles outside distance + 10 or after lifetime, using a four-second fade.

**Follow-up correction:** original ship centers at object +112 are (0,0,0),
not the camera. The previous port implementation added the camera's position
at spawn and recycle. That displaced the flight and changed apparent altitude,
scale and track as the player moved. Flights now stay in world coordinates.
The original default center is confirmed in all four live SkyObject groups.

Original RAM samples `original_t0.json` and `original_t1.json` cover 6.706667 s.
The two capital ships move 218.762 m and 299.721 m, measuring 32.619 and
44.690 m/s against stored speeds 32.650 and 44.732 m/s (original frame timing and
floating-point integration account for the small difference). The port uses
those authored speed ranges, with its own visual RNG stream.

Presentation choice: first visible scene frame starts with ships opaque, so
loading completion does not expose a fresh four-second fade. Recycling keeps
the original fade. This is not an assertion that the original spawn function
sets alpha to 1: it writes 0; our original save already has alpha 0.629833 after
2.519 s of pre-player scene activity. The initial opaque state is a shared
presentation choice, not a Geonosis ship exception.

## Fleet hulls and spawn RNG (player follow-up)

The player's PS2/openbf screenshots are retained locally as
`reported_original_ship.png` and `reported_port_ship.png`. The apparent broken
geometry was primarily depth compositing: ships used the dome pipeline,
which tests and writes no depth. Rear and upper faces could paint over the
nearer hull, and one ship could paint through another. The source mesh did not
need replacement to fix those exposed surfaces.

The earlier statement that lit ships were a deliberate deviation was wrong.
Dome draws use 0x20031; SkyObject draw 0x3bf060 instead passes
`caller_flags | 0x20001` (0x3bf0ec..0x3bf10c), while temporarily clearing the
composite/fog switch with 0x589860. That is not the lighting switch: 0x57f8e8
reads byte 0x60c2b5, observed as 1 in the running original. Materials retain
bit 0x01 lighting and opaque GS TEST 0x50000 (Z-tested/written); translucent
materials use 0x51001 (tested, no Z write). The original capital ship's three
source segments have flags 0x10000, 0x10001, 0x10001. The port now preserves that
unlit/lit distinction rather than lighting every part.

SkyShell now distinguishes depth-tested objects from background domes.
Opaque object parts write depth; translucent/fading parts test depth without
writing it and follow opaque hulls in far-to-near order. Their depth is also
shared with the world, so nearby terrain covers the ship while more distant
geometry cannot paint through it. Dome order, stars and dome depth remain
unchanged. This applies to every decorative fleet model, not just capitals.
The source asset census finds blended gunship parts on BF1 GEO1 and BF2
GEO1/COR1/MYG1; no baked sky-object material carries the cutout bit. The GPU
regression checks near/rear hull order, foreground/background world geometry,
transparent foreground parts over both hulls and opaque world geometry, and
dome effects beneath hulls, for both lit and unlit materials. Translucent sky
objects render with the blend stage after opaque world geometry, preventing
the world from overwriting their glass. Background dome effects paint before
three-dimensional hulls so their arbitrary angular display scale cannot place
an explosion over a foreground ship.

Two Acclamators are authored and live in the original. Their spawn routine has
no spacing or avoidance rule. The previous fixed per-object seeds happened to
produce a close pass: 16.590 m between fully visible capitals at 26.567 s in a
180-second trace. The original spawn routine consumes one RNG state at
0x73d868 across objects (loads/stores throughout 0x3bf270), rather than reseeding
each ship independently. The renderer now consumes a separate shared visual
spawn stream, kept independent of the simulation. Windowed battles seed it
from the map and a copy of the battle seed, without consuming or modifying the
simulation RNG; headless captures keep their repeatable presentation seed.
No minimum separation is imposed. Each logical ship advances
once regardless of the number of mesh segments; all its segments receive the
same resulting state. A unit regression compares one-part and multipart
versions through startup, pause, motion and recycling. It does not add collision
or avoidance for decorative ships. Exact original session paths remain
unsynchronized; their occasional proximity is not itself a defect.

`ship_depth_comparison.png` isolates depth handling at the same camera,
time and original port trajectories. `capital_motion_180s.csv` and
`capital_motion_shared_180s.csv` trace the old and corrected spawn streams from
the actual Flight implementation. These are trajectory diagnostics, not a
claim that the original imposes a minimum separation. The shared-stream trace
includes all 42 craft, since fighter/gunship recycling also consumes that stream.
The corrected trace helper uses the engine's exact byte-wise hash operation,
including `b | 0x20`; an earlier helper used ASCII lowercase and gave incorrect
trajectory numbers. `capital_motion_battle42_180s.csv` uses the windowed seed
for battle seed 42. Close passes remain possible, as in the original's rules.

`fleet_models_after.png` contains isolated close-ups of all three original
distant meshes. The source model audit (`sky_model_probe.txt`) finds 200, 99
and 74 triangles respectively, all stored outside bone space and requiring no
additional bind-pose transform. These are the original distant assets, not the
playable vehicle meshes. The capture helper preserves each segment's authored
lighting when restoring its shaded control.

## Distant explosions and lasers

`geo_sky_explosions` has eleven `spa_sfx_skydomeexplosions` attachments at
hp_sky_1, 4, 5, 7, 8, 9, 10, 11, 12, 14, 15. Named skeleton bind frames resolve
all eleven. The effect combines an Animated explosion atlas and additive Spark
laser fire (`com_sfx_laser_red`, from BF2 common).

Dome Effect's third/fourth arguments are repeat-delay bounds, not scale
(0x3bd604..0x3bd804); the Geonosis value 1 means a one-second repeat delay.
Dome RotationSpeed is an axis and turns-per-second rate: angle = rate x time x
2pi. Bake retains both delays, rotation, and hardpoint axes. The visual FX
system runs the authored emitters in dome coordinates, then applies the same
camera-relative shell transform as the model. Sky particles draw before world
depth and cannot affect collision, AI, input or weapons.

## Dust: BF2 FogCloud, not BF1 DustEffect volumes

BF2 Geonosis enables FogCloud with texture brownmist, Range 90..120, Color
168/172/180/128, Velocity 5/0, Rotation 0.1, Height 16, ParticleSize 28 and
ParticleDensity 100. Setter 0x462608 and live manager 0x173a560 agree with every
value. Geonosis SkyInfo FogRange is 0..0; adding global distance fog would
invent content. BF1's placed geo_prop_dust volumes are a different system.

Draw 0x474460 repeats one seeded pattern over 256 m XZ tiles. Each cloud sits
on the terrain height plus Height. The annular alpha is:

`max(0, 1 - ((distance_squared - (near_squared+far_squared)/2)
             * 2/(far_squared-near_squared))^2)`

The RNG seed is 0xc71c2831. X, Z and phase each consume two LCG steps; step 7's
bit 20 selects rotation direction, then step 8 is consumed. The earlier
rotation decision incorrectly used a newly assembled float; positions already
used the correct eight-step stride. A proposed seven-step stride was rejected
before delivery after checking the original's actual particle buffer.

Ten original packed particle centers were decoded from the VIF packet at
0x8abf04 using its recorded bounds at 0x8abd90. All match the eight-step stream
plus 12.609 m of X drift at 2.521889 s, within the approximately 0.003 m packing
precision. The port terrain query matches the sample cloud heights within
0.005 m. Their size is 3584/128 = 28; the shared VU1 sprite expansion gives
half-edge size/sqrt(2). This replaces guesswork about density and card size.
The bake now treats brownmist as an alpha consumer; it retains alpha 0..241
instead of an opaque square.

**Brightness correction:** original HDR scale 0x7c5754 is 0.5. FogCloud writes
vertex bytes (Color+1)>>1 once, e.g. RGB 84/86/90, while ordinary particles also
apply the HDR half in 0x5942f8. Thus FogCloud remains full-bright in the
half-bright scene and receives the final HDR x2. The old port treated it as an
ordinary display-space particle, making the dust too dark to obscure the
terrain. Cloud draws now carry that x2 display gain when the map enables HDR;
non-HDR maps retain gain 1. Density, alpha, texture and dimensions are unchanged.

The GPU regression uses brownmist's RGB 135/97/66 and authored vertex color:
on black at alpha 128, the HDR cloud must produce approximately 89/65/47,
against approximately 44/33/23 without the gain. Ordinary sprites must remain
byte-identical. Gain travels only with the cloud primitive; it does not change
ordinary particles, bolts or their lighting.

Original attribution captures: `emulator/snaps/...20261102095959.png` has
FogCloud, `...20261102100329.png` temporarily sets density 0, and
`...20261102100414.png` has density 100 restored. The low haze disappears with
the cloud count at zero. These are attribution captures with advancing time,
not exact same-frame pixel subtraction. The saved state supplies the exact
particle-buffer measurements above.

Other authored dust includes five geo_prop_splash props with five delayed
geo_sfx_explosion attachments each, and geo2_sfx_bunkerdust on bunkers. Existing
prop FX continue to run these independently from gameplay explosions.
BF2 END1/FEL1/KAS2/POL1/YAV1 also enable FogCloud; HOT1 disables it. Runtime
flight/FogCloud changes remain gated to BF2 until BF1's behavior is verified.
Other maps receive the data through normal rebakes, not map-name rules.

## Authored sky glare

The remaining broad sky difference exposed another missing data path, rather
than a resolution issue. `geo_sky_dome` has an HDR material with raw flags
0x10010. The existing GLOW RE (`analysis/re-glow.md` sections 1.1–1.3) identifies
this same path on GEO1, KAS2 and MUS1: alpha is a glow mask, not coverage.
The source material routine 0x58ed70 substitutes the self-lit color and applies
GS ALPHA 8, gain `1 + As/128`, with the original texture/vertex alpha conversion.

The bake's sky carve-out previously flattened `geo_sky` to alpha 255, despite
its source alpha being 0..115 with 96.188% exact zero. The HDR glow consumer
now retains that mask. A real-disc bake test compares the entire level-zero
RGBA payload to the original. Ordinary opaque sky consumers keep their old
rule; the texture is still drawn opaque.

The runtime previously sent every sky segment to `fs_sky` with LightPath::Lit,
ignoring its raw glow flags. HDR 0x10 and 0x14 now select the existing Glow and
Unscaled rules on sky shells too, without lighting or fogging the dome.
Sky bloom replays authored shell order and blend state at far depth against
the completed world depth. Later opaque sky art covers earlier glow, and
terrain occludes the source before the bloom blur. A GPU regression verifies
mask-zero brightness, masked gain, a visible halo, world occlusion and opaque
sky-layer occlusion. Nonemissive maps skip the extra sky gather entirely.
The existing selective-bloom approximation remains: ordinary lit whites and
translucent particle occlusion are not a full replay of the PS2 HDR frame.

New sky manifests record `glow_masks_preserved = true`. Older bakes default
to false and retain their prior sky rendering until refreshed; otherwise a
legacy all-255 texture on KAS2/MUS1 would incorrectly glow across the entire
sky. Schema tests pin that backward-compatible default. This avoids forcing
unrelated maps to rebake just to keep their previous appearance.

The partial refresh utility backs up and refreshes sky textures as well as
FogCloud, preserving unrelated map assets and foliage. Production Geonosis now
contains the source `geo_sky` mask. Other maps obtain corrected masks through
normal rebakes; their source data was not globally overwritten here.

## Shading, pause and evidence

Gemini's lit ship path is retained, with the source material lighting flags
restored as described above. Domes remain unlit, while lit ship segments use
sun/hemisphere/secondary lighting without distance fog.
The atmosphere clock freezes while paused; water has its existing separate
clock. Capture checks compare shaded/unshaded frames and demand byte-identical
output over 30 paused redraws.

`cargo run --release -p game --example atmosphere_capture -- <map> <out> <pose>`
produces 0..20 s sequences from a supplied
world pose. CAPTURE_HZ=10 captures ten frames per second; CAPTURE_NO_CLOUD is
an attribution control, and CAPTURE_FOV allows a reference projection check.
CAPTURE_SECONDS controls duration; CAPTURE_SHIP selects a logical craft,
CAPTURE_SHIP_DISTANCE moves the inspection camera closer, and
CAPTURE_ISOLATE_SHIP removes other craft only for the inspection captures.
Before executable and captures preserve the local Gemini state; after captures
use the same camera, asset bundle and prop-effect timing. Motion evidence is
under `engine/artifacts/geo_re/`; no copyrighted footage is committed.

## Remaining reference differences

The port is not a pixel replica of the PS2. Original ground combat is not
replayed by the decorative capture, and effect RNG seeds are not synchronized.
Original RAM supplied a measured camera near (-51.077,-60.540,173.669), pitch
-14.011 degrees; captures use the port's 52.05-degree vertical projection and
4:3 frame, with small remaining projection/aspect differences. The older
November 1 sky comparison is landmark-matched, not an exact pose dump.

BF2 Geonosis also enables Godray, Blur and MotionBlur. Eleven original godrays
are live (hangar, bunker and hull attachments); their missing draw is separate
from FogCloud and remains GODR. The PS2 Blur object's init/draw/property methods
at 0x465618/0x465620/0x465628 are stubs (`jr ra; nop`), so its enabled config is
not evidence for adding a global blur. MotionBlur's draw at 0x46cb20 reads a
separate per-view strength (initialized to zero), rather than continuously
softening the scene just because the map enables it. The independently authored
DomeInfo Softness pass does blend a blurred half-resolution copy at 60/128;
project comparison policy had explicitly excluded that SD softening. It remains
excluded here, consistent with the request to improve atmosphere beyond
resolution. Selective bloom, original combat smoke, unsynchronized effect seeds,
and that softening still prevent pixel parity. No extra global fog, BF1 dust
volumes or invented explosion emitters were added to hide these differences.
Dome effect offscreen kill/restart is not yet an exact replica of the original
lifecycle, although its authored placement, rotation, repeat delay and emitters
are restored.

## Validation completed

Maintainer playtest accepted on 2026-11-02: the packed `dist/openbf.exe` from
upstream 9a7d520 plus this feature was tested and reported working. Its embedded
release executable, all 365 Geonosis files and 61 mission files were verified
against the local build inputs before installation. Local packaging receipt:
`dist/openbf.exe.build.json`. This records visual playtest acceptance; the
independent source re-derivation status above is unchanged.

- Release game binary and capture example rebuilt; production Geonosis bake
  contains the motion/effect/cloud fields, brownmist alpha 0..241 and geo_sky alpha 0..115.
- `cargo test --release -p render -p manifest -p mungefs --lib`: 132 + 24 + 188
  passed; two unrelated corpus-dependent tests remain ignored.
- `cargo test --release -p game --lib fx::`: 78 passed, four unrelated optional
  chunk-asset tests ignored. Real-disc Geo sky/FogCloud and Hoth-disabled tests
  passed; bake count regression passed.
- `cargo test --release -p render --test suite sky_shell -- --ignored`:
  twelve GPU tests passed, including hull depth/transparency, sky glow/occlusion, cloud HDR color, unchanged ordinary sprites,
  lit sky objects, sky/world ordering, reflection and map clearing.
- Ten additional skinned GPU tests pass after integrating upstream's palette
  upload changes. Workspace all-target compilation also passes.
- The earlier lighting-only comparison changed 31,074 pixels; final ship
  captures additionally include depth, material lighting and shared spawning;
  30 paused redraws are byte-identical. Hoth's 21 surface-view sequence frames at (0,1,0), yaw/pitch 0, are all
  byte-identical to the pre-feature baseline; its no-ship and pause checks pass.
- `game.exe --sim-hash 100`: 5573f2b603ed95f4, the existing core test's pin.
  No simulation, input, collision, vehicle or AI files changed.
- Formatting, diff whitespace and provisional-value checks pass. Strict Clippy
  is not green on this upstream/toolchain: existing warnings in mungefs/shadow.rs
  (chunks_exact_to_as_chunks; also existing render/tests/suite/frame_filter.rs
  uses), core/droideka/ball.rs (needless_late_init),
  game/camera_collide.rs (map_or_identity), and game/roster.rs
  (needless_borrows_for_generic_args). A workspace all-target Clippy run with
  only those four lint categories allowed passes. No source lint allowances or
  unrelated fixes were added.

Reviewable local evidence under `engine/artifacts/geo_re/`:

- `ship_depth_comparison.png`: matched camera/time/trajectories before and after the hull-depth fix.
- `fleet_models_after.png`: isolated capital, gunship and fighter close-ups.
- `post_origin_motion/`: final combined-build 0..20 s sequence including shared fleet spawning and depth/material fixes; all 201 frames match `fleet_final_motion/`.
- `fleet_comparison.mp4`: before/after motion including the completed fleet rendering.
- `glow_comparison.png`: earlier atmosphere comparison before the hull-depth fix.
- `original_glow_comparison.png`: original / before / after wide view, landmark-matched (not exact original pose or effect seed).
- `glow_comparison.mp4`: earlier 6..20 s before/after motion; before at 1 Hz, after at 10 Hz.
- `glow_ram/`: atmosphere measured-original-camera dust sequence; glare is outside this lower view.
- `hoth_surface_before/` and `glow_hoth/`: identical surface-view regression sequence.
- `final_comparison.png`: earlier comparison before the sky-glow correction.
- `final_comparison.mp4`: 6..20 s before/after motion; before sampled at 1 Hz,
  after at 10 Hz, encoded at 30 Hz.
- `dust_before_after.png`: dust at the measured original camera, before/after.
- `original_dust_comparison.png`: original / before / after ditch-haze crop;
  original time 5.42 s, port 6 s. Remaining projection and timing limits above
  apply; the figure is not a pixel-parity score.
- `glow_motion/000.png` through `200.png`: earlier full 0..20 s after sequence,
  plus shaded/unshaded and frozen-pause capture pairs.
- `original_t0.json`, `original_t1.json`, `original_cloud_check.json`: original
  RAM motion, authored fields and decoded particle-buffer receipts.
