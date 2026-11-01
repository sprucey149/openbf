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

This first version is a **reference data export, not a runtime override**. The
game and bake do not read it yet. Its presence does not make Xbox-only imports
play identically, remove the current ELF dependency, or prove a complete import.
Generated tables preserve original values; the port's deliberate gameplay
deviations still apply at their existing layer.

## What may go public

`scripts/export_public.py` publishes `engine/crates/` and `scripts/`, so these files
go out with them. It adds a check of its own: only the generated data files on its
reviewed list may leave `engine/crates/manifest/data/` or `scripts/data/`, each within
a size budget. Each must be JSON whose strings are short ASCII names of at most four
words, with no 32-character run lacking a separator except a few whole SHA-256 hashes,
and with no block of more than 1,024 numbers, however it is nested or keyed. The one
exception is a table's `classes` lists, whose rows are checked as (sublevel, occurrence,
pool index). So a height grid, a vertex list, a key stream or encoded bytes are refused
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

Collision triangle meshes, terrain heightfields, navigation topology, skinned bind
geometry, hit geometry and complete animation tracks are asset geometry. Encoding
them as lists of numbers would still ship those assets, which is outside this table's
content rule. Counts, bounds and fingerprints can detect a discrepancy but cannot
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
BF1 also needs the PS2 BF1 disc. Allowing an Xbox BF2 disc later would satisfy the
ownership rule; it would **not** establish PS2 geometry equivalence.
