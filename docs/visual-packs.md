# Optional presentation packs

Unzip a prepared pack under `visual_packs/` in the repository root, then run the
normal build:

```sh
python3 scripts/pack.py
```

The layout must be `visual_packs/<pack-name>/pack.json`, with that pack's prepared
payload files below its own folder. Remove the pack folder and run the same
command to revert. The cached stock bake stays intact; adding, editing or removing
a pack does not require `--rebake`. An absent or empty `visual_packs/` keeps stock
packaging. This changes the packaged game; development runs reading the stock
bake continue to use that bake.

Use `--visual-packs <dir>` for a different folder containing pack directories.
Relative option paths are relative to your current directory; the default is
always relative to the repository. An explicitly named folder must exist.
Archives, preview images, authoring tools and directories without `pack.json`
are ignored and reported. A pack manifest is required to be valid, even if it
would match no assets. Ordinary builds need no network service, AI tool, Blender,
Pillow or additional executable to install prepared packs.

The build matches exact baked asset paths and accepted stock SHA256 hashes.
Missing targets and unfamiliar source variants are skipped, with counts and a
warning when a pack matches nothing. A replacement may cover several compatible
source profiles or map occurrences. It does not automatically apply to similarly
named files, alternative texture tiers or distant meshes. Two matching mappings
for the same target are an error; folder ordering never chooses a winner.
Errors stop before replacing the existing packaged game.

`<output>.sources.json` records each installed or skipped target, stock path,
accepted and actual stock hash, replacement digest, pack name, manifest digest,
ignored material and counts. Console output gives a short summary. Pack matching
and validation run again on every build, including a cached bake. Only named,
present originals are hashed, once per path. Do not edit inputs during a build.

## Pack manifest v1

```json
{
  "version": 1,
  "replacements": [
    {
      "file": "textures/one.ktx2",
      "targets": [
        {
          "path": "bf1_rhn2/textures/name.ktx2",
          "sha256": ["<64 hexadecimal characters: SHA256 of accepted baked original>"]
        }
      ]
    }
  ]
}
```

`file` is relative to the pack root. Each target `path` is relative to
`assets_baked`, without an `assets_baked/` prefix. Use forward slashes on all
platforms. List additional accepted hashes in `sha256`, or additional paths in
`targets`, only after checking those variants' layout and interpretation.
Unknown fields, empty replacement/target/hash lists and malformed JSON fail.
Absolute paths, traversal, Windows drives or stream names, reserved device names,
directories and links outside the pack root fail. Pack directory symlinks are
ignored. Only files explicitly named in the manifest are installed.

## Texture author requirements

Use the native KTX2 container, under an existing `textures/` path or a map's
`load/` path. The runtime currently reads raw RGBA8 as sRGB. Pack validation
therefore accepts only uncompressed, straight-alpha, 2D RGBA8 sRGB (vkFormat 43,
BT709), one face, no array layers, the bake's basic RGBA descriptor, no extra
orientation/swizzle metadata and a complete correctly sized mip chain down to
1 by 1. Dimensions must preserve the original aspect ratio and be at most 8192
on either axis. Compressed GPU blocks, Basis, linear/normal-map reinterpretations
and renamed PNG/JPEG files are refused. Alpha-bearing textures must preserve the
original dimensions and **every alpha sample at every mip**; opaque textures may
change resolution while remaining fully opaque at every mip.

Preserve UV and atlas layout, orientation, palette, intended material use and
alpha meaning. Validation cannot determine whether new RGB artwork respects
those requirements. Prepare and review mip filtering offline. A higher-resolution
sheet cannot repair incompatible UV layouts between source variants. Cover
`.tier2.ktx2` and `.dtl.ktx2` separately where appropriate. PNG is currently
excluded: no baked PNG presentation targets have been demonstrated in this scope.
RON, scripts, audio, missions, class data and collision cannot be replaced.

## Static mesh author requirements

The first audited path is
`bf1_rhn2/models/rhn_bldg_catacombs_wall.glb`. Other mesh paths require a separate
consumer audit before being added to the loader's allowlist. No side/render
model currently has that proof: soldiers, heroes, vehicles, weapons, attachments,
skins, animation, terrain, contact surfaces and collision meshes remain excluded.

The file must be a self-contained GLB 2.0 with one internal binary buffer, one
rigid mesh and indexed opaque triangle primitives. External buffers/images,
embedded images, glTF extensions, skins, animations and morph targets fail.
Preserve the baked nodes and scenes, mesh name, all mesh extras (especially
`sphere` and `vis_top`), ordered primitive material bindings and **complete baked
material metadata** exactly. Preserve original bounds within 0.000001 metres.
Positions, normals and UVs must be finite and well formed; indices must stay in
range and any vertex alpha must remain opaque. Mesh density can change.

The game binds textures using baked material **names**, including map/common
registry resolution and texture tiers; an embedded glTF image is not a substitute
for a matching KTX2 replacement. Exporters often strip the bake's material/light
extras and gameplay/culling metadata. Restore those fields during preparation,
remove unused image/PBR references, and validate the final GLB with the native
loader. Do not simply rename an exporter result.

Check the actual baked draw binding on each profile. Some mixed-source bakes draw
a separately named model; a matching path/hash records a file substitution, not
proof that it is selected in every scene or LOD. Other LODs remain stock. The
collision archive entries and manifests remain stock, so relief must stay within
the established surface bounds. Bounds and metadata checks do not prove visual
alignment, silhouette fidelity, coverage, LOD transitions or gameplay acceptance;
review them headlessly against each source and then in play before accepting a
pack.

Keep prepared packs and all game-derived art local. This repository ships only
the loader, validation, synthetic checks and authoring instructions.
