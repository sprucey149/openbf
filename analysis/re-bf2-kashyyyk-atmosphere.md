# BF2 Kashyyyk: sky glow, low mist and water colour

Current integration note (2026-11-04): #118's shared atmosphere and subdivided
flat-water drawing are present at `4cda5ab4`. The original template RNG state,
reduced far-water template and original-game visual sign-off remain unverified.
The drawing changes do not alter the canonical wet mask or gameplay geometry.

[O] UNAUDITED. Source configuration, existing water RE and local renderer
captures; no independent re-derivation or synchronized original-game comparison.

The maintainer supplied PS2 and port images looking across the beach from the
sea wall. The local black box is dist/openbf_data/playtest/last.json: BF2 KAS2
conquest, seed 66, exit tick 1275, camera (40.44, 30.08, -94.89), yaw 191.941,
pitch -8.594 degrees. The build identifies 04b156bcf572 plus edits, while the
checkout at battle start was 6d3bd2339d81. The snapshot therefore does not
represent a clean 6d3bd23 binary. Its camera is used to reproduce the port view.

## Sky and mist

Kashyyyk's legacy cached manifest lacked the newer sky glow preservation flag,
FogCloud and decorative flight fields. The shared Geonosis/Hoth rendering paths
already support them. Refreshing the player's PS2 KAS2.LVL restores the alpha
glow mask of kas2_sky without replacing its RGB. The source material's glow
flag and authored HDR settings produce the bright horizon and surrounding bloom.

The authored FogCloud uses the common texture fluffy, Range (40,80), white
RGBA (255,255,255,75), Velocity (4,0), Rotation 0.1, Height 9, ParticleSize 18
and ParticleDensity 20. The ordinary world fog is disabled. No distance fog
blanket or additional sun flare is added to approximate the screenshot.

The partial refresh tool now accepts atmosphere textures supplied by the baked
common group. It previously tried to copy fluffy from the map-only bake, where
that shared texture does not exist. Local backups are retained. The refresh
also installs the freshly baked water mesh and its matching manifest record.

## Water

The previous mesh had two triangles per 128 m wet cell and used the diffuse
table's average RGB everywhere. re-water-colour.md section 1.5 establishes the
original template's entries 5..15: Min + (Max-Min)*(index-5)/5. KAS2 authors
Min RGB 30 and Max RGB 70, giving eleven values 30..110, rather than uniform 70.
PatchDivisions (4,4) and INFO5 4 produce a 16 by 16 subdivision per wet cell.

The bake now stores that spatial diffuse variation as vertex RGB for flat
water with valid subdivisions and no CPAT patches. The template repeats with
matching opposite edges. KAS2 retains the same 42 wet cells, heights, UV scale,
water texture and simulation wet mask, while the drawing grows from 84 to
21,504 triangles. The manifest marks vertex diffuse data explicitly, so runtime
does not multiply it by the old average a second time. Legacy meshes keep their
previous behavior. Oceans and CPAT sheets retain their existing paths.

Speckle still uses its own ambient/specular colours and animated source sheets;
it must not inherit the new base-layer vertex RGB. Its texture repeat uses the
authored wet-cell size rather than the smaller drawing subdivision.

The exact random template realization is provisional and visual-only. This
implementation preserves the documented LUT range and periodic topology, but
does not reproduce the original RNG state. Far water still uses the near
template rather than the original reduced template. Alpha retains its previous
mean; KAS2's authored minimum and maximum alpha are both 255. This is not a
claim of exact original-game water rendering or all-map visual acceptance.

## Evidence and limits

Local captures, logs and refresh backups are under
engine/artifacts/kashyyyk_atmosphere/. The atmosphere capture tool now steps
water along with effects and weather. Ship-lighting visibility is optional,
because Kashyyyk's decorative flight can be outside the captured view; the
explicit CAPTURE_REQUIRE_SHIP_LIGHTING check remains available. Frozen redraws
hold every presentation clock and require identical pixels. The earlier
atmosphere-only freeze also advanced material clocks, so it was unsuitable
as a whole-frame frozen-water assertion on this map.

Verified: 64 renderer GPU referees, 32 water integration tests, 24 schema tests,
11 water bake unit tests and the real-disc Kashyyyk sky/mist/water regression.
Normal Clippy completes with existing upstream warnings; this is not a strict
CI-green claim. The provisional-value and diff-whitespace checks pass.
The size/map script fails under Windows path handling; a normalized diagnostic
finds two pre-existing unmapped source files, outside this change.

The same-camera 1280x720 release scene benchmark on the RTX 4050 Laptop GPU,
Vulkan, measures 4.34 ms mean, 4.17 ms median and 6.90 ms p99 over 120 frames.
It has one unit and no skinned draws, and is not a full-battle performance test.

The refreshed local map is packed into dist/openbf.exe. The executable prefix
matches the release binary and 1,302 files covering Kashyyyk, Geonosis, Hoth,
missions and the shared fluffy texture match disk byte-for-byte. The packed
Kashyyyk map loads and captures successfully. Both loose and packed binaries
retain simulation hash 5573f2b603ed95f4 at 100 ticks. The previous executable
and metadata are backed up under the local evidence directory.

Full source alignment acceptance, an all-map visual sweep and a synchronized
PCSX2 water comparison remain outstanding. No game assets or captures belong
in Git. Simulation, collision, AI, weapons and gameplay water queries are not
changed by these rendering edits.

## Water display blending follow-up (2026-11-03)

The local playtest snapshot now ends at tick 852, seed 204. Its checkout is
80ec6ee952c2 but its binary identifies 193078918b7e plus edits. The local
loose KAS2 bake also still had 84 triangles without vertex diffuse colours.
Refreshing that map from the player's PS2 LVL restores the branch's 21,504
water triangles and existing sky glow. The water texture retains alpha 158;
its RGB, the authored diffuse/specular values and the gameplay wet mask are
not retuned.

The remaining renderer deviation was display arithmetic. Base water multiplied
separately linearized texture and tint, then blended against a linearized bed.
Speckle added linear light instead of the stored display colours documented
in re-water-alpha.md and re-water-colour.md. The water and speckle runs now
use a non-sRGB view of the current world target, including scaled targets.
Base modulation, source-over transparency and the two weighted speckle frames
operate in display values. The base still writes depth; speckle still reads
depth without writing it. Later grass, blended geometry and particles resume
their existing targets. A surface that cannot expose a non-sRGB view keeps
the linear blend fallback.

Local evidence is under engine/artifacts/water_fidelity/: before is the stale
map, refreshed isolates the asset refresh, after adds display blending, and
after_cloud includes the existing atmosphere at the latest playtest camera.
The water remains animated and frozen redraws remain identical. These captures
demonstrate the local changes, not synchronized PS2 parity. Exact water colour,
shoreline appearance, per-vertex Fresnel interpolation, original template RNG
and the original far-water template still require original-game comparison.

Validation: all 66 renderer GPU referees and 134 renderer unit tests pass;
render Clippy passes with warnings denied, as do formatting, whitespace and
provisional-value checks. Both loose and packed release executables retain
simulation hash 5573f2b603ed95f4 at 100 ticks. The packed Kashyyyk map loads
and captures successfully. No synchronized PCSX2 comparison, full workspace
CI, full source alignment acceptance or all-map visual sweep was run.

dist/openbf.exe was replaced only after staging and checking the completed
pack. It contains all 24,574 current engine/assets_baked and engine/missions
files. Its executable prefix matches the release binary; all missions plus
the Kashyyyk manifest, water mesh, base texture, sky, 25 speckle frames and
shared fluffy texture match disk byte-for-byte (91 contents checked). All
72 existing dist/openbf_data files retain their sizes and modification times.

## Broad colour variation follow-up (2026-11-03)

The maintainer's next comparison correctly showed that the first correction
still lacked the PS2's large light/dark areas and cooler distant water. The
paused native slot 10 state now supplies direct evidence; see section 8 of
re-water-colour.md. Its 289 near and 25 far colour/phase indices all match
the recovered fixed-seed LCG and column order. The bake now uses that sequence
instead of the provisional xorshift template. Ten flat-water maps were
rebaked through refresh_atmosphere --water-only, preserving the existing
atmosphere. Water height, wet mask and gameplay flags match their backups.

The renderer reconstructs the far template from the initial unwrapped native
sequence and interpolates its triangles over the authored cell size. KAS2
uses 32 m far colour cells and its 85..100 m near/far transition. This retains
the near grid instead of stretching one mean colour across the sea.

The native reference also has ordinary world fog disabled but retains a
90..350 m fog matrix. The far-water path does not consult the ordinary fog
flag. Its RGB (104,106,108) is the map's authored SkyInfo fog colour. The bake
now preserves that raw colour separately, and BF2 KAS2 uses the measured
reference range as a stateless water-only correction. This deliberately
matches the supplied state rather than reproducing history-dependent fog
left by arbitrary prior missions. Fresh-boot and prior-map-order traces are
still outstanding; 90..350 is not claimed to be KAS2's authored FogRange.

Water fog is folded into the finished water/bed composite algebraically,
including alpha, instead of fogging only the water source before blending.
Speckle is attenuated by that same fog amount. World fog remains disabled.

Local before/after images at identical camera, FOV and time are
after_cloud/000.png and comparison_after/000.png under water_fidelity/;
water_comparison.png shows their common water crop. native_haze/ uses the
eye recovered from the native water constants. reference_view/ uses the
native fog matrix's heading and an approximate display FOV; it is not a
synchronized pixel-registration acceptance capture. The source state and
running emulator were left intact and paused. The zero-water bed experiment
was prepared but not loaded after Windows menu targeting failed.

The 68 renderer GPU referees, 134 renderer unit tests, the targeted water-haze
rule and 24 schema tests pass. The existing water integration and bake checks
were also rerun. Clippy completes with two existing warnings in bake/src/dlc.rs;
the changed game and renderer code introduces no Clippy warnings. Formatting,
whitespace and provisional-value checks pass. Full workspace CI, full source
alignment and an all-map visual acceptance sweep remain unclaimed.

The release executable was rebuilt and packed with all 24,574 current files.
Its prefix matches the compiled binary, and 113 embedded contents including
all water meshes/manifests, all missions and KAS2 water/sky textures match disk.
The packed KAS2 scene captures successfully and retains simulation hash
5573f2b603ed95f4 at 100 ticks. The completed staged pack replaced dist/openbf.exe after the game closed.
dist/openbf_data was not replaced or removed.

## Pale-band contrast follow-up (2026-11-03)

The next comparison exposed another missing amplitude. A fresh native GS
frame confirms the HDR composite ALPHA 0xff00000029 and FIX 255. Flat-water
VU overlays ignore the ordinary HDR half-colour command; their surface and
speckle contributions consequently receive 255/128 gain at final composite.
Section 9 of re-water-colour.md records the GS states and palette evidence.
The renderer now applies that display-space gain to BF2 HDR flat water RGB,
preserving alpha, the existing water fog and ordinary world lighting. This
is a runtime shader correction, so the current baked assets need no refresh.

Identical-time/camera captures contrast/before_sea/000.png and
after_sea/000.png show the stronger pale bands. water_contrast_comparison.png
contains the corresponding unaltered crops. In the fixed water rectangle
(400,260)..(800,300), a two-pixel Gaussian blur followed by subtraction of
each row's mean isolates lateral band variation from the horizon gradient:
its luminance p95-minus-p5 grows from 14.47 to 30.22 display levels. This is
local before/after evidence, not synchronized PS2 pixel parity. No full-map
visual sweep or full workspace CI was run.

Validation passes: 69 GPU renderer checks, 134 renderer unit tests and 34
water integration checks. Clippy completes with the same two existing
nonminimal_bool warnings in bake/src/dlc.rs; denying all warnings fails on
those unchanged lines. Formatting, whitespace and provisional-value checks
pass. The release game and capture example were rebuilt. All 24,574 current
asset/mission files were packed; archive inventory, executable prefix and
113 selected embedded contents were verified. The staged executable loads
and captures Kashyyyk and retains simulation hash 5573f2b603ed95f4 at 100 ticks.
It successfully replaced dist/openbf.exe. All 74 dist/openbf_data files retain
their recorded sizes and modification times. No source assets or binaries
were added to Git.

## Dark-patch and dither follow-up (2026-11-03)

The reported view was recovered from the BF2 KAS2 seed 207 exit snapshot
(build 80ec6ee+dirty, tick 843). The camera is (41.22, 30.44, -94.64),
yaw 188.503°, pitch -14.897°. At that pose and the same water clock, the
corrected sampling and far-speckle rounding lower water luminance p10 from
67.21 to 60.36 and p90 from 87.93 to 85.07 in the fixed rectangle
(525,265)..(848,315) of the 856×642 frame. The local before/after crops are
in water_fidelity/far_alpha/water_precision_comparison.png. The shader and
GS measurements are in re-water-colour.md section 10.

A second comparison uses the camera solved from the captured PS2 water
vertices, with position (10.7125, 29.9355, -105.3377), yaw 111.73462°,
pitch -18.95042°, vertical FOV 52.06484°. The tile offset is identified from
the shoreline and the estimated height uses the base water plane. This
comparison still shows residual differences. The left water rectangle's
mean luminance is 76.44 against 59.39 on the PS2, while the right rectangle
is 88.70 against 84.58. Full pixel parity and the full alignment gate were
outside the checks performed for this local refresh.

The renderer passes all 71 GPU checks, and the 34 water integration checks
pass. Clippy completes with the existing two bake/src/dlc.rs warnings.
The release executable was rebuilt and packed with the current 24,955
asset and mission files. This runtime change uses the existing bakes.
The archive inventory, binary prefix and 113 embedded contents match disk.
The packed executable renders Kashyyyk and yields simulation hash
5573f2b603ed95f4 at 100 ticks. The checked pack replaced dist/openbf.exe.
A normal playtest ran during this work and updated the profile and snapshot
rotation. The install operated on the executable path only and preserved
all existing distribution data paths.
