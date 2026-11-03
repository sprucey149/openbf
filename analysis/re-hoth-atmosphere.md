# BF2 Hoth: blowing snow, sun and lens flare

[O] UNAUDITED. Local disc/ELF reads and PS2 reference captures; no independent
re-derivation. This implements the maintainer's Hoth atmosphere trial, not
DUSTE/SUNG acceptance across all maps.

## Integration and reference

The follow-up first fetched origin/main at b436e41 and merged it into
codex/hoth-atmosphere in 8163f2d. The existing Geonosis work and Hoth changes
were retained. Both conflicts were crate-map documentation. The pre-update
patches and stash remain available locally.

The next update merged origin/main 5281236 in 04b156b without conflicts and
restored the pending atmosphere work. The combined workspace passes all-target
checking. The later PS2 density investigation confirmed the same 80-particle
snow pool, eight-particle mist pool and 59 active placements. No further density
or height tuning was made; the maintainer deferred that comparison.

The reported build's black box was read from
dist/openbf_data/playtest/last.json: BF2 Hoth conquest, seed 60, build 5d8f895,
exit tick 567, camera (49.34, 2.41, -2.29), yaw 3.39, pitch -0.29 radians.
The maintainer's 2.57-second video shows broad low white snow banks, separate
from falling flakes. The additional PS2 screenshot shows glare washing over
foreground geometry when looking toward the sun.

Reference inputs are the player's PS2 HOT1.LVL, USA v2.01 executable and
Hoth conquest. Local disassembly, RAM reads, captures, probes and builds stay
ignored under engine/artifacts/hoth_atmosphere/. No original assets or
decompiled text are published.

## Blowing snow

Snowfall already exists: Precipitation has 80 particles per tiled cell,
velocity 2, range 15, wind base (4, 0, 0.3). FogCloud is disabled on this map.
Adding flakes does not restore DustEffect.

Conquest places 52 hoth_prop_dust and seven hoth_prop_mist volumes. They have
no model; the old scene upload skipped them. Runtime reads their properties
from the existing classes.ron and shares one particle pool per class.

| Property | Blowing snow | Mist |
|---|---|---|
| Texture | whitedust | hoth_mist |
| Spawn minimum | -40, 0, -40 | -15, 1, -15 |
| Spawn maximum | 30, 15, 30 | 15, 6, 15 |
| Velocity minimum | 2, 0, 2 | 0, -0.5, 0 |
| Velocity maximum | 10, 1, 10 | 0, 0.5, 0 |
| Size | 15..20 | 10..18 |
| Lifetime | 2..3 | 3..6 |
| Alpha | 1 | 0.5 |
| Shared pool count | 80 | 8 |
| MaxDistance | 150 | 100 |
| Radius fade | absent | 10..30 |
| HeightScale | constructor 0.2 | 0.8 |

Constructor 0x1bd570, setter 0x1bd800, spawn 0x1bced0 and step 0x1bcd58
supply the class and lifetime rules. Alpha fades over the first 30% and last
50% of life. Radius fading uses horizontal distance.

The follow-up checked original class/pool memory and vertex writer 0x3c3988,
called from 0x3c4310. The original instance matrices have identity X/Z and
scaled Y, despite the authored placements' reversed X/Z axes. Wind therefore
moves in world axes. The original expands with camera right and up, then
scales world Y for both centers and corners. Upright cards and rotated wind
in the first prototype were wrong, particularly when looking down.

Original class bounds at +156 are 109.10087585 for snow and 39 for mist.
0x1bdc70 uses max position norm + max velocity norm * max life + half max size,
or RadiusFadeMax + half max size when radial fading is enabled.
0x1bca38 adds this bound to the origin's forward depth, rejects outside
MinDistance/MaxDistance and fades over the last 200 m. Hoth's MinDistance is
zero. This remains a placed, intermittent effect; no global storm blanket,
extra volumes, inflated counts or altered opacity were added.
CameraDistance behavior on other maps remains outside this trial's validation.

0x3c3cb8 sets unscaled white RGB and the volume fade. Together with the
particle colors, NORMAL 0x44 blending and HDR's half-scale frame, the final
dust source color has the same full-bright convention as FogCloud.
The native display target clamps unassociated RGB before alpha blending:
multiplying white by two alone loses this gain. AtmosphereQuad therefore
premultiplies RGB by alpha in the shader and uses premultiplied source-over.
Opacity, depth testing and particle ordering remain authored. Other particle
types retain their existing pipelines.

The bake recognizes dust sheets as alpha consumers; partial refresh retains
backups and preserves the rest of the existing map. Level-zero alpha ranges
are 0..104 and 0..243. Opaque legacy sheets are skipped with a rebake diagnostic.

## Sun and optical glare

The root SunFlare scope in fx__ is distinct from Effect("SunFlare") and BF2's
unused SunInfo. Parser 0x46e998 reads it. Hoth authors Angle (115, -20), white
Color, Size 5, FlareOutSize 20, NumFlareOuts 40, InitialFlareOutAlpha 70,
SpikeColor (230,230,0,128), SpikeSize 20 and these halo rings:

| Radius | RGBA |
|---|---|
| 0 | 255,255,255,255 |
| 10 | 255,200,0,255 |
| 30 | 255,127,0,0 |

Direction is (sin(a)*cos(b), -cos(a), sin(a)*sin(b)); the original live object
holds (0.8516352, 0.4226177, -0.3099753). Original HDR scale is 0.5.
0x46f798 places the source at min(0.9 * FarSceneRange, 2000): Hoth's 800
gives 720. Sun/halo radii scale by distance/120.

The first prototype's sinusoidal rays and eight small colored circles were
not the original optical geometry and have been replaced. Reads of
0x46fdc0, 0x46f840, 0x471b90, 0x472050 and 0x472500 establish:
- 30-sided core/halo rings, with GS byte halving and interpolated RGB/alpha;
- successively enlarged copies of the 20-sided source mask, using the authored
  count and alpha curve InitialAlpha * (1 - i/count)^3, stopping below byte 3;
- two triangular spike fans, using SpikeSize, the projected source position
  and the original 0.0111, 0.0025 and phase constants;
- thirteen fixed colored polygon/ring reflections with original centers,
  radii, colors and side counts, instead of arbitrary hues or circles;
- a broad white 30-sided glare fan. Its extent follows
  35 * distance * 0.0111, with an outer radius three times the middle radius.
  Its alignment factor is max(0, (facing - cos(fov_h/2))/(1 - cos(fov_h/2))).
  Original Hoth RAM confirms middle alpha 72 and inner alpha zero.

A local instruction probe recorded the optical routine's ring arguments and
cross-checked its constants. The native renderer emits polygon rings and
triangle fans on the CPU, as the original does; the shader interpolates their
RGB and alpha. An initial fullscreen evaluator took 29.30 ms/frame at 1280x720
on the RTX 4050 Laptop GPU (120-frame bench), so that representation was
replaced before delivery. The source is no longer rejected
just because its center is outside the viewport. Out-of-frame depth samples
are invisible. Core and halo respect depth at their own pixels; optical
reflections can overlay the foreground. Sun light also enters the existing
HDR gather before the authored bloom cascade. No camera-dependent exposure
or additional whole-frame brightness multiplier was invented.

The PS2 reference camera is (12.230314, 1.223096, -12.849821), yaw -95.64443,
pitch 10.02416 degrees. The original was saved in previously unused slot 3;
existing slots 1, 2 and 10 were not overwritten. Native captures at this pose
show the same broad bright source and colored reflection arrangement.

The PS2 reduced source-mask filtering under partial occlusion has not yet
been reproduced exactly. The native depth filter is explicitly provisional,
with the comparison against 0x470ce0 named in its comment. Sampling,
fixed-point rasterization, SD softness, particle seeds and combat are not
synchronized. Do not describe this as a pixel-identical original-game match.

## Verification

The follow-up capture tool includes the existing weather alongside dust and
world effects, renders each 30 Hz tick before selecting output frames and
checks identical pixels over 30 paused redraws. Earlier dust-only captures
did not include the existing snowfall and were incomplete combined previews.

The final triangle path passes all 63 GPU referees and both focused sun checks.
Renderer library tests pass (134), schema tests pass (24), and the real Hoth
parser, bake/transparency and active-placement regressions pass. Workspace
all-target checking, formatting and normal Clippy complete; Clippy retains
existing warnings under Rust 1.99, so this is not a strict CI-green claim.
The provisional-value check passes. The 100-tick simulation hash remains
5573f2b603ed95f4.

The final triangle build's same-camera 1280x720 bench is 1.99 ms mean,
2.02 ms p50, 2.71 ms p99 on the RTX 4050 Laptop GPU, Vulkan, over 120 frames.
An aligned sun view at 1920x1009 measures 2.05 ms mean and 2.94 ms p99.
This is a scoped scene benchmark with one unit and no skinned draws, not a
full-battle bot-count or integrated-GPU performance acceptance result.
Final sun and moving-bank captures include weather and pass the pause check.
The terrain-offscreen probe was rejected as invalid camera evidence; the final
bank sequence uses the checked above-ground pose (49.34,4,-42), yaw 194.23,
pitch -12 degrees, and an attribution capture disables only DustEffect.

The initial trial was dist/openbf-hoth.exe. The maintainer subsequently requested
the combined build at dist/openbf.exe, including the updated main UI. Its cached
Geonosis atmosphere data was refreshed from the local disc before packing.
Packing verifies the release executable prefix and all 822 Hoth, Geonosis and
mission files against disk, including both dust alpha ranges. Both maps load
and render from the packed executable; all 63 GPU referees pass again after
the second main merge, and the packed 100-tick simulation hash is unchanged.
The latest local receipts are under engine/artifacts/hoth_atmosphere/density/.

These are shared systems driven by source configuration, with no Hoth or
Geonosis map-name switches. The current bake refresh covers those two maps;
other maps can consume their own settings on rebake. BF2 Kamino, Polis Massa
and Yavin IV have DustEffect classes, but their current legacy opaque sheets
are skipped pending rebake. DustEffect and FogCloud runtime support remains
BF2-only. Shared sky and decorative flight changes also affect other maps;
an all-map visual comparison was not run.

No build, capture or game asset is staged. Full DUSTE/SUNG map sweeps,
independent verification, all-source alignment acceptance and complete original
source-mask filtering remain outstanding. Simulation, bot visibility, weapons,
collision and simulation RNG are unchanged by the atmosphere feature.
