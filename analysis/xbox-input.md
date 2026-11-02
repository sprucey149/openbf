# Xbox discs as input

Status 2026-11-01. Both Xbox discs now read through `mungefs` and bake. What that
does and does not give a build is below; all counts are measured on the local
trimmed discs.

`scripts/pack.py` takes them: `XBOX_SUPPORTED` in `scripts/input_scan.py` is true. A disc standing alone bakes through
the public instruction tables in `manifest/data/gameplay/corrections-v1/`, which turn its files into the PS2's numbers and
shapes, and a changed file refuses the bake. `analysis/source-gates.md` has the proof per map and the maps left out. The
drift table below is what the disc holds before those corrections.

## Readers, and how each was proved

| Format | Where | Check on real data |
|---|---|---|
| XDVDFS | `mungefs/src/xdvdfs.rs` | Lists 169 and 219 files; every LVL read back equals an independently extracted copy (128 and 164 LVLs) |
| `tex_` | `mungefs/src/texture_xbox.rs` | All 6,267 `tex_` chunks decode. Same-size PS2 twins: median difference 0 on swizzled L8, A8 and A8L8 |
| `modl` | `mungefs/src/model.rs` | 1,174 segments with a PS2 twin: 1,125 in the same place, normals off on 0 of 393,429 vertices, uvs on 297 of 392,995, top bones on 0 of 158,216. Winding: 310,300 against 310,305 of 331,083 triangles face their normal |
| `tern` patches | `mungefs/src/terrain.rs` | GEO1 heights equal. Layer nibbles within 2 steps: 95 % and 94 %. 22 of 23 HOT1 cutter patches alike. Lightmap orientation: r 0.293 against -0.047 transposed |
| Sound banks | `mungefs/src/soundbank.rs`, `bake/src/adpcm.rs` | 536 Geonosis samples decode to their declared frames. Loudness curves of 174 of 202 PS2 twins correlate above 0.9 |
| Streams | `mungefs/src/stream.rs` | 12 BF1 Geonosis music tracks through the baked index and `audio::TakeReader`, against the PS2's: each decodes to its declared frames, loudness r 1.0 on both channels. Multi-substream takes were not compared |
| Shell movie | `bake/src/shell.rs` | XMV segments transcode through ffmpeg |

Format 4 on the Xbox is the IMA block layout the PC's streams call format 5, so
the reader gives both one name. `0x0E` is the DXT3 block and is straight, not
premultiplied: 616 of 641 twins with partial alpha match that reading.

## The two ways to bake

**With the PS2 disc.** The map bakes from the PS2 LVL and the Xbox LVL only offers
larger copies of the same painting to tier 2. Nothing the sim reads can come from
the Xbox: on all 34 maps every gameplay section matches the PS2's print, and
1,800-tick headless traces on BF1 TAT1, BF1 GEO1, BF2 GEO1 and BF2 KAS2 are
byte-identical to a plain PS2 bake. The tree given as the PS2's has to earn it: an
Xbox tree is refused outright, and any other tree whose prints or class files
differ, such as a PC install, is refused like a drifting map. Tier 2 gains 37 BF1 and 344 BF2 textures. BF1 gains little here, because its larger Xbox art sits on
detail-compressed PS2 bases, which the tier rules keep.

**Alone, without corrections (`bake xbox`).** The map bakes from the Xbox LVL, with its larger art and its own
numbers and shapes. This breaks the gameplay rule on most maps:

| Section | BF1 maps drifting (of 16) | BF2 maps drifting (of 18) |
|---|---:|---:|
| collision | 16 | 14 |
| classes | 14 | 8 |
| instances | 12 | 5 |
| heights | 11 | 1 |
| regions | 7 | 8 |
| layers | 0 | 8 |
| paths | 1 | 4 |
| nav | 2 | 1 |
| cutters | 2 | 1 |
| water level and wet cells | 2 | 1 |

Only BF2 DAG1, MUS1 and MYG1 match in every map section, and the side class files
drift as well. Of 1,137 collision meshes on both discs, 989 are the same bytes, 14
are the same triangles reindexed, 95 are different shapes, and 39 exist on one disc
only. So `bake xbox` refuses such a map unless told `--allow-gameplay-drift`.

The sections are hashes, shipped in `manifest/data/gameplay/ps2-prints-v1.json` and
made by `bake xbox-prints` from the PS2 discs, plus each map's foliage as plain counts
(`bake check-foliage` holds any baked tree to those). They hold no meshes, art, audio or
text. A hash detects a difference and cannot supply the missing value. They are also
not everything the sim reads: the door and turret rigs in the manifest's classes and
the model GLBs `game::stations` measures are not hashed, because they also hold what
only draws.

A matching trace does not clear a drifting map: a 600-tick trace of BF2 GEO1 alone
equals the PS2's although its collision and regions differ, because nothing in
those 20 seconds touched the difference.

## Calling it

```sh
bake extract-xiso <disc.iso> <dir>                # <dir>/Data/_LVL_XBOX/...
bake xbox <disc.iso|dir> <out> [--ps2 <root>] [--maps a,b] [--allow-gameplay-drift]
```

`bake xbox` tells the game by content and refuses a source that does not hold Xbox
LVLs. It writes `<out>/<map key>/` under rebake.sh's map keys, and writes
`<out>/xbox_report.ron`. A disc image is extracted once under
`<out>/.xbox_source/`, in a folder of its own, so both games can share `<out>`. It exits non-zero when it refuses a
map, after writing the report. Every map is covered; the groups below are not.

Every other `bake` subcommand reads an Xbox LVL as it reads a PS2 one, so the
common, side, classes, load, sounds, global-sound, beep and shell groups run from
an Xbox data root. All of rebake.sh's groups pass that way, but only map bakes and
class files are compared with the PS2. Soldier clips, vehicle rows and weapon nodes
from an Xbox side are not gated. A donor argument (`bake map <ps2.lvl> <out>
<pc.lvl>,<xbox.lvl>`) takes a comma-joined list. A folder name may hold a comma: pieces
are joined until they name a file, and an entry that never does fails with a read
error.

## Known gaps when the Xbox stands alone

- **Water is flat.** The Xbox's `Effect("Water")` names normal maps and no
  `MainTexture`, so the port's water draws one colour on, for example, BF2 KAS2.
- **Fog and sky objects are the Xbox's.** Its `sky_` adds a fog range and extra
  sky objects. These change the look only.
- **Terrain paints at most 3 layers per patch**, where the PS2 paints up to 5.
- **`_bump` textures bake but nothing draws them.** That needs a renderer path.
- **Some A8 colour is a list of names.** A8 drops colour. Four surfaces are known
  to be black from their PS2 twins; any other black one with no twin would read white.
