# PS2-style vertex lighting in the world renderer

2026-11-04. Baseline `8d4481ab6ef38e18cd3c87ab9914e249d95f1a33`, with the
preceding Felucia material/detail/horizon corrections in the working tree.
The maintainer requested implementing the per-vertex lighting difference
identified in `re-felucia-surfaces.md`. [O] UNAUDITED: existing original PS2
static evidence and new port regressions; original matched-frame acceptance
is separate.

## Evidence and scope

`re-model-lighting.md` section 2 establishes the BF2 VU1 directional term
at instructions 0x1e6-0x20d and the hemisphere ambient at 0x1bb-0x1d2. Both
are evaluated per vertex before COL0 is applied. Section 3 establishes the
prelit set's static-light exclusions. `re-vertex-colour-interp.md` sections
3-5 establish screen-space interpolation of the completed vertex colours,
including the clipping constraint. Its section 8 explicitly retained the
port's per-pixel directional/ambient lighting as an unmeasured difference.

The previous shader interpolated a normal, normalized it in the fragment
stage and evaluated directional/hemisphere lighting there. It also blended
COL0 and local-light sums independently before combining them. Normal
normalization and the diffuse clamp make this visibly different on curved
low-poly plants and across shadow transitions.

## Implementation

`shader.wgsl::vertex_lighting` now evaluates the existing material lighting
formula per vertex. It preserves each path's light selection, BF1 sun curve,
character ambient/regions, static-light gates, local lights and established
colour-transfer formula. The completed material lighting is encoded to
display units and interpolated with the existing guarded affine weights.
The fragment stage decodes this interpolated lighting and modulates the
sampled texture/detail; texture coordinates retain perspective correction.

The shared varying replaces the previous local-light-only varying, so no
additional inter-stage attributes or new GPU resources are needed. Lit,
Character, Prelit, PrelitAdd, VertexLit and interface paths use it. Unlit,
glow, terrain and water keep their own existing shading rules. The existing
linear texture-modulation approximation is retained; this change does not
claim a complete bit-exact GS reconstruction.

Rigid meshes and skinned meshes use the same evaluator, with skinning applied
before lighting. The specular shader evaluates its base through the same
vertex result rather than reconstructing per-pixel lighting. Its skinned
variant also decodes the character light region, keeping the specular
clipping delta consistent with the visible base in a lit room.

This is a runtime-only change using the current baked normals, colours and
textures. It adds no plant-specific lighting multiplier and no postprocess
blur. The original Felucia softness pass remains a separate presentation
choice. The core simulation and source assets are unchanged.

## Validation

The new GPU checks use varying normals/colours and unequal vertex depths so
per-pixel lighting, linear-light interpolation and perspective interpolation
produce different results. The skinned fixture rotates each vertex normal
around a fixed vertex position, verifying lighting after posing. A second
check adds a fixed specular contribution over the same Gouraud base for
rigid and character-region-lit skinned models.

Diagnostic captures include a close Felucia plant, Coruscant and BF1 Theed.
A newer Felucia exit snapshot (seed 227, tick 3100) provides the reported-view
camera (56.91, 16.14, -110.54), yaw -108.289 degrees, pitch -26.929 degrees.
The older packed build supplies the before frame at that exact port pose.
These comparisons isolate the port change; the supplied PS2 plant crops
have different poses and do not establish exact original-game acceptance.

Full cross-source alignment acceptance and per-map 60 fps bot populations
are not measured by this change. Final commands, counts and packed-build
verification follow below.

Final focused checks:

- `scripts/gpu_tests.sh --render`: 81 passed, headless, one test thread.
- `cargo test -p render --lib`: 135 passed.
- `cargo build --release -p game`: completed after the shader and character-region packing changes.
- `cargo fmt --all --check` and `git diff --check`: passed.
- The size checker was run with Python 3.10 because `python3` is the Windows Store stub. It reports all source rows missing due to backslash paths, plus two pre-existing absent documentation targets (`engine/artifacts/briefs/SPLIT_RECIPE.md`, `engine/artifacts/loop/gates.toml`). A separate normalized source-path check confirms all 747 source modules have their own map rows. Touched counts were refreshed. The unmodified repository size checker itself therefore remains failing on this host.

Matched port before/after RGB mean absolute changes, on a 0..255 scale:
Felucia close plant 1.582, reported Felucia view 0.322, Coruscant overview
0.256, BF1 Theed overview 0.164. Mean signed changes are -0.543, -0.049,
-0.244 and -0.125 respectively. Visual inspection shows changed curved
light/shadow transitions with little whole-frame brightness movement.
These figures measure the implementation's effect, not PS2 parity or
frame-time performance. The final CPU character-region packing fix affects
skinned draws, so the static captures remain representative.

The current assets need no rebake. The final release executable was staged
with all 24,955 current baked assets and missions through `game.exe --write-pack`.

The staged executable rendered the reported Felucia pose using only its
embedded asset paths; that PNG is byte-identical to the unpacked release
capture. After this smoke check it replaced `dist/openbf.exe`. All eleven
profile/playtest files retain their pre-pack SHA-256 hashes. No baked assets,
reference images or packed executables were added to Git.
