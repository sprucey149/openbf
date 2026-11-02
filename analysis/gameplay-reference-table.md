# One gameplay reference across disc editions

PS2 is the reference. Selecting another edition's texture or model must not
select its health, weapon tuning, mission population, collision, terrain or
animation timing. The measured differences and coverage are in
[version-diff.md](version-diff.md); the input inventory is in
[version-input-dependencies.md](version-input-dependencies.md).

## First generated version

`engine/crates/manifest/data/gameplay/` holds names and numbers only:

- `foliage-v1.json`: both games' verified 256-entry placement lookup and grass
  atlas UV coordinates. It includes the original executable SHA-256, virtual
  addresses and the existing bake reader's checksums. No executable byte range
  is stored. `scripts/gameplay_foliage.py` regenerates it and refuses an unknown
  table, unsupported ELF architecture, ambiguous segment or truncated input.
- `ps2-v1/manifest.json`: reference game and platform, the census and table hashes,
  and how many definitions, distinct records and left-out values each game has.
- `ps2-v1/bf1.json`, `ps2-v1/bf2.json`: a `class_pool` of distinct class records (kind,
  name, base, ordered property-hash/value pairs), and for each source LVL the
  sublevel, occurrence and pool index of every definition, plus mission names and
  direct, branch-free `ScriptInit` declarations under the chunk's initial literal
  state. Numeric vectors stay numeric arrays. Unsupported values stay `NM`.

`scripts/gameplay_table.py` generates these from PS2 censuses. It selects explicit
fields and never copies the census's opaque sections or input files. The file,
sublevel and occurrence identity prevents a side class from overwriting a map-local
override. Repeated properties keep their order. A property hash is not a resolved
name; its numeric identity is authoritative.

The table leaves out by name what the simulation has no use for: every `Label` (text
a player reads, which the census now also marks `NM`), voice and sound cue names,
effect and texture names, colours and HUD art references. `PRESENTATION_NAMES` in the
generator is the list and the manifest counts what it dropped. Numbers whose purpose
nobody has named stay in. With the class pool this takes the table from 5.5 MB in 79
files to 1.7 MB in 3.

Those dropped values pick a sound, an effect or a colour, so they belong with the
donor's own assets: only the donor has the banks they name. `core` carries some of
them as cue names, so a consumer of this table still has to take them from the donor,
not leave them empty.

The source parser's static mission coverage and unresolved arguments remain
limitations, not permission to invent runtime values. A null or `NM` is not zero.
Helper side effects and `ScriptPostLoad` execution are not modeled; affected
fields remain `NM` where identified. These declarations are not authoritative
evaluated missions. The broader `static_calls` evidence is excluded from this
table, and complete executed mission values remain unmeasured.

The original `ps2-v1` files remain reference exports. The correction revision
below derives the donor-specific overrides consumed by the bake. A table by
itself does not establish an equal world or remove other source dependencies.
The port's deliberate gameplay deviations still apply at their existing layer.

## What may go public

`scripts/export_public.py` publishes `engine/crates/` and `scripts/`, so these files
go out with them. It adds a check of its own: only the generated data files on its
reviewed list may leave `engine/crates/manifest/data/` or `scripts/data/`, each within
a size budget. Each must be JSON whose strings are short ASCII names of at most four
words, with no 32-character run lacking a separator except a few whole SHA-256 hashes,
and with no block of more than 1,024 numbers, however it is nested or keyed. Two narrowly typed exceptions are reviewed: a reference table's `classes` lists
are (sublevel, occurrence, pool index) references, and `corrections-v1` terrain
`height_samples` are finite (world x, world z, absolute target height) triples
strictly checked against the accompanying playable boundary polygons. Only the
four exact donor-table paths receive that height exception. File and private
resource fingerprints also have explicit typed fields. A full height grid, vertex
list, key stream, out-of-bounds height correction or encoded bytes is still refused
whatever the file is called, in those folders. These are shape checks: numbers mixed
with names, a small mesh under the block limit, or text split into short pieces would
still pass, and generated data outside those two folders is not covered. It does not
replace reading a new table before adding it to the list.

Regenerate into a fresh output directory, review its diff, then replace the
versioned table deliberately. The generator refuses stale/foreign output files
instead of silently leaving definitions from an earlier source. Example:

```sh
python3 scripts/version_census.py scan PS2_BF1_DISC_OR_FOLDER --out bf1.json
python3 scripts/version_census.py scan PS2_BF2_DISC_OR_FOLDER --out bf2.json
python3 scripts/gameplay_table.py bf1.json bf2.json --out reference-tables
python3 scripts/gameplay_foliage.py --bf1 SWBFRONT.ELF --bf2 SWBFRNT2.ELF --output foliage-v1.json
```

## Integration contract

1. Identify the game/platform by container contents; filenames are not identity.
   Pin the census/table schema and reference revision in the baked manifest.
   Reject an unknown revision or unresolved required gameplay field.
2. Resolve gameplay classes and mission rules from the PS2 reference before
   applying port-specific tuning. Resolve visuals from the selected donor.
   A missing class or scope must be an explicit import error, never a fallback
   to that donor's different gameplay values. Preserve the existing class
   inheritance, first-definition precedence and ordered-property behavior.
3. Supply the numeric foliage lookup through a validated table loader. Keep the
   existing EE rounding and placement algorithm; matching random values alone
   does not guarantee matching positions.
4. Keep all geometry and motion that affect simulation on a separately verified
   canonical path. Compare decoded inputs and resulting bake outputs before
   allowing a new edition as its source. Record failed and unmeasured categories.
5. Prove integration with focused reference/donor bake comparisons and a fixed
   input simulation hash on every supported source combination. Do not infer
   this result from a census or a render that looks similar.

## What a numeric table cannot repair

The full list, with the terrain and collision measurements and the options for a
build with no PS2 disc, is in [version-diff.md](version-diff.md).

Collision triangle meshes, navigation topology, skinned bind geometry, hit geometry
and complete animation tracks remain asset geometry. Encoding them as lists of
numbers would still ship those assets, which is outside this table's content rule.
The maintainer approved **only terrain height corrections inside playable bounds**
on 2026-11-01. This permits neither a whole heightfield nor outside-boundary
samples, polygon patches, vertices or meshes. Counts, bounds and fingerprints can detect a discrepancy but cannot
reconstruct the shape. Do not silently use donor collision or interpolate a missing
root-motion track to satisfy an equality claim.

Animation names, durations and endpoint displacement can be tabulated. They cannot
reproduce intermediate movement, contact timing or pose-dependent hit shapes when
those tracks differ. Mission populations and object identities do not supply missing
map meshes. Content in only one edition needs its actual assets or an authored
substitute.

Fonts, HUD artwork, controller icons, shell backgrounds, movies, sounds and localized
prose also need source assets or newly authored replacements. Tables can specify
layout coordinates and name mappings, but cannot make differing art identical. See
the dependency inventory for measured missing names.

The current input policy requires a console BF2 disc with any PC input and refuses
PC-only builds. `scripts/pack.py` accepts only a PS2 BF2 disc for that today, and PC
BF1 may use its fingerprinted corrections and locally generated geometry without
a PS2 BF1 disc once their complete bake checks pass. The Xbox input switch stays
closed until its readers land. Allowing an Xbox BF2 disc later satisfies the
ownership rule; it does **not** establish geometry equivalence.

## Absolute correction revision

`scripts/gameplay_corrections.py` generates four reviewed `corrections-v1` JSON
files, one per non-reference game/platform pair. Each LVL records its exact source
SHA-256. The bake verifies that fingerprint before any correction and refuses an
unknown revision with the file and corrective action. A PS2 source returns through
the original path without loading a correction table.

The generator emits complete absolute target values for changed classes, mission
declarations, fog/sky config, object/region/hint/barrier records, primitives, model bounds and
cutters. Missing target records are inserted and donor-only records removed.
World ordering is reconstructed from explicit original-donor and target-record
indices; it does not depend on hash-map iteration. Repeated properties and scoped
identities survive. Presentation properties come from the donor, and unresolved
properties are explicitly preserved and remain unmeasured.

Height corrections include **all** differing grid vertices inside a PS2 boundary,
including terrain under objects and cutters. They are absolute heights, never
platform deltas. Missing boundaries do not authorize a correction. The generator
leaves outside samples alone. A terrain triangle crossing the boundary may still
read an uncorrected outside vertex; the comparison must report that edge case
instead of claiming exact in-bounds queries from matching interior vertices.

Changed collision meshes, navigation resources, skeletons and root motion leave as instruction
lists over the player's own file, in `<game>-<platform>-programs.json` beside each table. Nothing else
is read: the build needs only public code, these tables and the player's copy. `scripts/gameplay_programs.py`
writes the lists and `bake::gameplay::program` applies them.

- **Collision** reuses before it writes: runs of donor vertices, translated runs, one-axis moves, midpoints of
  donor edges, then absolute coordinates. Polygons are copied from the donor through vertex positions, or listed.
  What the game reads is reproduced exactly (NAME, MASK, NODE, bounds, vertices, polygons in order). The tree's split
  data and each leaf's six unread bytes are written as zero.
- **Skeletons** are donor bones by index, a donor bone with new numbers, or a name, parent and numbers.
- **Navigation and paths** are donor word runs and written words.
- **Animation** keeps the donor's clips. Only root tracks are replaced; a clip with other frames or bones is written
  whole. Other bones keep the donor's pose, so a pose the simulation reads from a non-root bone would differ: the
  tick-by-tick rounds are what check that.

The generator applies each list and compares it with the reference before keeping it, and the bake refuses a result
whose SHA-256 differs from the table's row, so a list cannot quietly substitute donor geometry.

Height fills cover ground past the one-cell line that a query from inside the bounds can still read. The reach is the
hover gun's 512 m aim ray, tested as a square around each grid sample, so it is a little generous at the corners.
Each fill is a rectangle of one height; BF2 Yavin 1's 8,283 such rows take 853. BF1 tables hold many more (Hoth 5,378
rectangles, Endor 1,703; about 240 KB a table, against about 26 KB for BF2's).

The exporter decodes the lists, reads the ones its name check skips field by field, and caps what may be written or moved
outright per table. A region name passes only as a shadow
setting. `engine/private-gameplay/` is still refused.

Animation banks are selected by decoded root-curve and clip-clock
differences, including constant offsets whose later subtraction changes float32
results; bank packing and renderer-only bone poses do not select a replacement.

`gen_missions.py --gameplay-reference TABLE --gameplay-source GAME=SOURCE`
validates the donor `MISSION.LVL` fingerprint and applies the same revision's PS2
facts before existing port tuning. The verified donor supplies presentation cues
when that game's PS2 script is unavailable. Unknown required declarations fail;
unknown helper/callback execution remains unmeasured. This preserves the distinction
between static mission declarations and complete evaluated mission behavior.

Focused and cross-source bake checks are required before claiming equality. A
successful export scrub, a known fingerprint or an applied correction is not that
proof. Mesh publication remains unapproved; this cache must never be exported.


## Comparing baked worlds

`scripts/world_fingerprint.py ASSETS MISSIONS MAP --out FILE` hashes a conservative
projection of every baked simulation input, plus fog/sky and environment settings.
It covers map and shared collision, boundary-cell terrain support, nav sidecars,
ordered class properties, missions, vehicles, blast spheres, and saber skeletons,
poses, motion, combos and weapon nodes. It excludes render vertex resolution,
textures and audio. Some environment and class-rig fields are presentation-only;
a component difference must be classified before calling it a physics difference.
Missing saber dependencies mark the result incomplete. `game::world_fingerprint`
refuses incomplete or unknown identities and compares the small source-independent
identity; the engine build identity remains a separate future handshake check.

`version_parity_check.py` compares independently staged source combinations and
runs identical seeded headless proxy rounds, checking state hashes every tick
and at the outcome. Missing inputs, incomplete fingerprints and unfinished rounds
never count as identical. A matching round is bounded evidence, not proof of every
possible input or of an unvisited boundary query.
