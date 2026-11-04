# Felucia plant shells, ground detail and distance haze

2026-11-04. Source baseline: `8d4481ab6ef38e18cd3c87ab9914e249d95f1a33`.
[O] UNAUDITED: original static PS2 evidence,
maintainer screenshots and port GPU regressions; no new matched PCSX2 capture.

## Evidence

The maintainer supplied PS2/port pairs showing opaque bladder plants, weak
fine ground variation and overly distinct distant silhouettes. The latest local
F9 snapshot initially held BF2 Dagobah at tick 973. A later snapshot from
BF2 Felucia, seed 225, tick 16369, supplies eye (-38.98, 14.79, -54.2), yaw
-0.1 and pitch -0.15 radians. Both snapshots identify the older packed build
`1166c50b5b91+dirty`, while the checkout is `8d4481ab6ef3`. The later camera
is retained for a headless haze inspection; it is not a matched PS2 pose.

[V] The original PS2 `FEL/FEL1.LVL` model `fel1_bldg_bladder` has three segments:
`0x10098` inner glow, `0x10001` solid base and `0x85` outer shell. All use the
same texture. `segdump` reports 125/94/296 vertices and 127/56/308 decoded
triangles respectively, with 801 declared indices matching the model total.
The shell texture has mid fraction 0.24868774, low fraction 0.017425537 and
maximum alpha 255. The previous census made its specular/translucent shell
opaque, so it wrote depth and hid the glow.

[V] In `analysis/SWBFRNT2.ELF-decomp.c`, `FUN_0058ed70`, lines 711150-711216,
material bit 0x04 selects translucency independently of the specular flag or
texture histogram. Ordinary translucency uses source-over and no depth write.
The bake now follows that rule for BF2 words without the cutout bit; words
carrying 0x02 keep their existing treatment. This is a material correction,
not an object-name exception. The inner glow remains its existing HDR path.

[V] The same LVL carries `tern/DTLX = fel1_det02`; its `DTEX` payload is zero.
`FUN_00570f68`, lines 693838-693899, selects the three by three terrain patches
around the eye and applies the detail texture multiplicatively. The `0x89`
blend and full vertex alpha produce frame times twice the texture alpha
(`re-terrain-lighting.md`, section 6). `FUN_005700a8`, lines 693464-693507,
packs patch-local position fields from 0 through 65535. The separate texture
coordinate stream at lines 693495 and 693528-693559 advances by 0x800 per
cell, the terrain's 1.0 ST unit (`re-terrain-layers.md`, section 3). Thus the
detail texture repeats once per grid cell, eight times across a patch; the
patch-selection size remains eight times the grid unit (32 metres on Felucia).
This texture is a contrast multiplier, not a PBR roughness or normal map.

[V] Felucia authors `NearSceneRange(0.1, 28, 52)` and `FarSceneRange(0)`.
`FUN_0058ae10`, lines 709082-709145, maps the range endpoints to reversed,
reciprocal view-depth keys. `FUN_00584838`, lines 705468-705509, fills the
palette with a linear ramp between those keys. The continuous near-scene
weight is `(1/z - 1/end) / (1/begin - 1/end)`, clamped to zero through one.
At 40 metres it is 0.35; the previous smoothstep returned 0.5. Depth is planar
view depth, so a plane has the same fade at the image centre and its sides.

## Implementation boundaries

The manifest retains the DTLX name and patch size as optional render data.
The bake preserves the detail texture alpha, including where no model names
that texture. BF2 terrain bindings carry it separately from ordinary model
textures. The shader modulates terrain display colour within the selected
patches before conversion back to linear colour. Each painted layer uses the
same world-space multiplier, so source-over layers retain the same detail
pattern without multiplying the already composited frame repeatedly. BF1
rendering is unchanged; its metadata is retained for future use.

The near-scene fade now uses reciprocal planar depth for terrain, models,
skinned models, water and specular contributions. It remains a per-fragment
approximation: PS2 depth-byte quantization and the single completed near/far
buffer composite are not implemented. Overlapping translucent surfaces can
therefore still differ. PS2 display softness is deliberately unchanged.
A complete matched-view haze acceptance remains outstanding.

## Validation

See the retained local logs `engine/artifacts/felucia-gpu-tests.log` and
`engine/artifacts/felucia-rebake.log`. Commands and final counts are recorded
below after the final build. Asset files, screenshots and packed builds remain
local and ignored by Git. Full cross-source alignment acceptance and per-map
60 fps bot populations are unmeasured by this work.

GitHub reservation was attempted, but the CLI rejected assignment with missing
`ReplaceActorsForAssignable` permission. No remote issue update was made.

Final focused results:

- `cargo test -p bake --lib alpha::tests`: 14 passed.
- `cargo test -p manifest --lib`: 25 passed, including the appended optional field and old-manifest compatibility.
- `cargo test -p game --test suite glb_material -- --test-threads=1`: 3 passed, including the shell/glow round trip.
- `scripts/gpu_tests.sh --render`: 79 passed, serial/headless. Numeric checks cover world-space detail repeats, display-colour modulation, the near-patch boundary and reciprocal planar-depth haze.
- `scripts/rebake.sh maps tat1 tat2 common`: 34 map bakes and both common groups completed; foliage check reports 94,486 instances, 21 grass classes, zero lost or differing.
- `cargo build --release -p game`: completed after the final shader and terrain binding changes.
- The size checker ran through Python 3.10 because this Windows host's `python3` command is the Store stub. Correction recorded during the vertex-lighting follow-up: its native-Windows path separators cause false missing-row reports, and two pre-existing documentation targets are absent. Normalized source-map coverage is complete. `git diff --check` passed.

Local captures: `engine/artifacts/felucia-shell-final.png`,
`felucia-ground-final.png` and `felucia-haze-final.png`. Ground comparison
`felucia-ground-no-detail.png` uses the same camera and corrected haze without
terrain detail; its scratch bake lacks common props, so it is a terrain-only
diagnostic. `felucia-haze-before.png` uses the previous packed executable and
the later Felucia F9 camera. The final ground uses the original 256 by 256
detail texture with retained alpha 0..255 (mean 100.719), repeating per four
metre cell, rather than inventing texture data or adding PBR parameters.

The final release executable packed 24,955 files from the current
`engine/assets_baked` and `engine/missions` into
`dist/openbf.felucia-staged.exe` (1,877,791,610 bytes). Its headless render
from virtual `assets_baked/bf2_fel1`, without a disk asset directory in its
working directory, is byte-identical to the unpacked final ground capture.
After successful packing and this smoke check, the staged executable replaced
`dist/openbf.exe`. All seven existing profile/playtest files under
`dist/openbf_data` retain their pre-pack SHA-256 hashes.

## Horizon-silhouette follow-up (2026-11-04)

The seed-226 Felucia exit snapshot (tick 1694, build `8d4481ab6ef3+dirty`)
locates the reported view at (-42.91, 15.72, -52.36), yaw -0.46, pitch -0.28
radians. The repeating striped pods are painted into the separate
`fel_sky_dome_trees` sky panorama, not world models leaking through the
near-scene fog. The DTLX and plant-shell corrections remain independent.

[V] The PS2 LVL names that panorama as the third DomeModel, with material
0x05 and diffuse RGBA 255 throughout. Its texture is 256 by 128 with authored
opaque tree regions and transparent sky above. `FUN_0058ed70` lines
711150-711216 selects source-over for this material. The sky draw flag
excludes the layer from ordinary fog (`re-sky-pass.md`, sections 0 and 2).
Adding world fog to this shell would not reproduce the original rule.

The original far target is lower precision and separately composited;
`re-sky-pass.md` section 0 lists those remaining differences. The current
renderer displays the authored panorama with much stronger visible outlines
than the maintainer's PS2 reference. At the maintainer's request, the explicit
`SKY_LAYER_TUNES` table applies opacity 0.2 only to BF2 Felucia's
`fel_sky_dome_trees`, through the existing sky blend path. Every other sky
layer and every world object retains its previous opacity and fog distances.

**PROVISIONAL reference correction, not a recovered PS2 material value.**
measure: obtain a matched PS2/port sky-only frame at the seed-226 diagnostic
camera and replace this opacity factor with a measured far-target/composite
implementation or a fitted contrast correction. This follow-up deliberately
makes the distant panorama less noticeable, as requested; it does not claim
full PS2 framebuffer parity.

Follow-up validation: all 13 `viewer::look::tests` pass. The release game
renders the seed-226 camera into `engine/artifacts/felucia-horizon-after.png`;
`felucia-horizon-before.png` retains the previous packed build at the same
pose. Visual inspection confirms the striped skyline is faint. The lower half
of the two captures is byte-identical, and the fixed horizon rectangle
(480,140)..(725,230) changes by mean absolute RGB 2.382 display levels.
No asset rebake is needed for this runtime-only sky-layer correction.

The follow-up release packed all 24,955 current asset/mission files to a
staged executable. Its embedded-asset capture from a working directory with
no unpacked assets is byte-identical to the final release capture. The
verified executable replaced `dist/openbf.exe`. All nine existing
profile/playtest files retain their pre-pack SHA-256 hashes.

## Yellow-plant shading inspection (2026-11-04)

The maintainer asked whether the darker, more solid-looking plant shading is
an error or a consequence of sharp presentation. This follow-up inspects the
existing implementation only; it does not change runtime code or refresh dist.
The latest available F9 snapshot is still seed 226, tick 1694; its camera does
not establish a matched pose for the newly supplied plant screenshots.

[V] The yellow stalk and petal atlas is `fel1_bldg_whisker1`, 256 by 256.
`fel1_bldg_whisker_clump01` has a lit, opaque 0x10001 body and a lit, cutout
0x10003 petal segment. Neither has COL0. These material inputs do not request
translucent trunks, self-lit surfaces or a missing PBR material. The textures
already carry fine bark/fibre colour detail that a sharper display reveals.

[V] Felucia intentionally authors SoftnessParam 65. As established in
`re-glow.md` section 4, BF2 `FUN_00586578` blends the finished image with a
blurred half-resolution copy at 65/128, about 51 percent. The port deliberately
does not apply that legacy softness. This explains part of the softer fine
detail and edges in the PS2 photographs; it is not a darker-lighting rule.

[V] There is also a real, previously documented lighting difference:
`re-vertex-colour-interp.md` section 8 and `re-model-lighting.md` section 2.
The VU1 directional term (instructions 0x1e6-0x20d) and hemisphere ambient
(0x1bb-0x1d2) are evaluated per vertex, then the GS blends the resulting
colours. In the port, `mesh_vertex` passes normals and `shade_unfogged`
normalizes and lights the interpolated normal per fragment. Local point
lights already have their own per-vertex path; they are not this difference.

[D] A static arithmetic probe of the actual opaque plant mesh confirms that
this distinction can darken shadow transitions and brighten lit portions.
At triangle centroids of placed `whsk_clump1096`, using the runtime's column
basis and Felucia sun vector, 32 of 318 triangles have a directional term
more than 0.05 lower under per-pixel lighting, while 30 have one more than
0.05 higher. The extrema are -0.19479 and +0.34777 sun units, mean absolute
0.03117. This isolates normal interpolation and the diffuse clamp; it is not
a complete-frame brightness measurement or original-game acceptance.

Conclusion: blur/resolution and a genuine lighting-model difference can both
contribute. The unmatched camera and light-facing orientations in the supplied
crops prevent assigning a percentage to each or proving an overall brightness
offset. No plant-specific brightness or opacity tune was introduced. A future
faithful lighting correction should evaluate the directional/ambient result
at vertices and retain the GS interpolation rule, with matched PS2 evidence.
The local playtest executable and profile/playtest files are unchanged by
this inspection.
