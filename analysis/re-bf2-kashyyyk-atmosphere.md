# BF2 Kashyyyk: sky glow, low mist and water colour

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
