# Inputs that a platform-neutral build still needs

Measured 2026-11-01 against the original five editions in [version-diff.md](version-diff.md).
This is a source dependency audit, plus a targeted UI/content inventory. It does
**not** establish equal gameplay from different editions. Code references were
rechecked against `main` after the PC readers of
[PR #94](https://github.com/sprucey149/openbf/pull/94) were merged. That merge also
made the policy stricter than the PR at that checkpoint: PS2 supplied gameplay
and BF2 shell/HUD data. The 2026-11-01 correction path can bake fingerprinted PC BF1
gameplay from public instruction tables, and its input gate is on, leaving out the maps it cannot
draw (`analysis/source-gates.md`); the console BF2 dependency remains.

Three different problems must remain separate: an asset name is absent from a
particular container; an equivalent asset exists in a different encoding or
under another name; or a reader cannot consume it yet. None alone proves that
an entire edition lacks that content. Counts below concern the named containers,
not an unbounded search of every DLC, language or revision.
The [numeric receipt](../scripts/data/version-diff/ui-dependencies.json) lists
the measured missing resource names and localization-key IDs per container,
font metrics, movie segment metadata and source hashes. It contains no text
translations, font artwork or media.

## Shell, HUD and fonts

- **BF2 shell movie:** PS2 `MOVIES/SHELL.MVS` has 34 indexed segments, all with MPEG
  pack signatures. Xbox has 33, all beginning with an Xbox video header (the
  bytes `xobX` at offset 12); compatibility with the current transcode is **NM**.
  PC has four Bink segments. The index and four-byte payload signatures were read
  directly, without decoding or copying media. `bake/src/shell.rs` accepts MPEG and
  Bink. Four segments cannot reproduce the PS2's 34 by changing a number. The
  actual menu selects only a subset: `shell_main` and `shell_sub_left`.
- **Shell backdrop:** PS2 and Xbox `SHELL.LVL` have `iface_instant_action`; PC does
  not, and `bake_shell` stops on a missing `SHELL_TEXTURES` name. PR #94's substitute
  did not survive the merge, because the shell now comes from a console disc. All
  seven names exist on Xbox, which also adds `iface_instant_action_wide`. The two PS2 DNAS logos are absent from Xbox/PC
  `SHELL.LVL`, but `bake_shell` does not request them. PC also localizes the Dolby
  logo name. Those logo differences are not gameplay dependencies.
- **HUD layouts:** BF2 `INGAME.LVL` contains five `hud_` chunks on **each** of
  PS2, Xbox and PC. Presence is measured; equivalence of all records is **NM**.
  `bake/src/hud.rs` reads the layout plus texture names from `COMMON.LVL` and
  `CORE.LVL`. `bake/src/hud_pc.rs` adjusts the stock PC single-player anchors and
  fonts to the PS2 coordinate convention. The
  [visual-parity report](pc-import-visual-parity.md) says that PC font art remains
  different and multiplayer layouts are not normalized.
- **Missing PC control sheets:** PS2 `COMMON.LVL` has
  `btn_directional_pad_d`, `_l`, `_lr`, `_r`, `_u`; PC's corresponding container
  lacks those five names. The bake explicitly cites `_lr` among layout-driven
  dependencies. Xbox has those names, but seven other PS2 names are replaced by
  localized variants: `btn_analog_l`, `btn_analog_l_down`, `btn_analog_r`,
  `btn_analog_r_down`, `btn_r1`, `btnl1`, `btnr1`. Alias resolution or newly drawn
  control icons is needed wherever a requested name fails; a table cannot supply
  the icon pixels. The 18 explicit `SCREEN_SHEETS` names are present on all three.
- **Fonts are present, not missing:** BF1 PS2/Xbox `CORE.LVL` each has five font
  names and 226 glyphs per font. BF2 PS2/Xbox each has four; PC has five (the
  additional name is `gamefont_super_tiny`). Every BF2 font also has 226 glyphs.
  BF2 large/medium/small/tiny line heights are PS2/Xbox **22/18/18/15**, versus PC
  **22/19/17/13**. The PC medium/small/tiny page counts are **2/2/1**, versus
  PS2/Xbox **3/3/2**. Font metrics fit a table; page artwork does not.
  `mungefs/src/font.rs` delegates each page to `Texture::parse`, which takes PC FMT
  textures since PR #94. Xbox texture dispatch is not on `main`.
- **Localized text:** all five editions' `CORE.LVL` contain six `Locl` languages.
  English record/unique-key counts are BF1 PS2 **2057/2053**, Xbox **2236/2207**;
  BF2 PS2 **4722/4717**, Xbox **4893/4820**, PC **5259/5054**. Duplicate hashes are
  counted separately from records. BF1 PS2 has **10** English keys absent from
  Xbox; BF2 PS2 has **97** absent from Xbox and **114** absent from PC. These are
  key-set comparisons, not text equality measurements or counts of keys used by
  the current UI. `bake_hud::bake_locl`/`mungefs::locl` consume text for event
  lines, team/post/class names and menu labels. A names/numbers table can retain
  key IDs, but **must not contain the localized strings**. A console donor or
  independently written replacement text is required for missing requested keys.

## Numeric tables read from the executable

The production bake's direct ELF-byte reader is `bake/src/foliage.rs`:

- `rng_table`: 256 float32 placement values; BF1 virtual address `0x5453c8`, BF2
  `0x636ac8`; both measured FNV-1a32 `0xf1fdba48`.
- `grass_atlas`: 17 four-corner UV rectangles, 136 scalar coordinates in total;
  BF1 `0x518948`, BF2 `0x5ffb20`; source block length `0x110`, measured FNV-1a32
  `0x000be48d`. The four lookup cases are NumParts 1, 3, 4 and default, containing
  4, 3, 4 and 6 parts respectively. Each source coordinate is a u16 in units of
  1/2048; the generated artifact stores interpreted normalized UV coordinates.

Both tables were freshly read from **both** PS2 ELFs and compared equal. They are
now represented as numbers in
[`foliage-v1.json`](../engine/crates/manifest/data/gameplay/foliage-v1.json), with
source executable SHA-256 values and virtual-address/checksum provenance.
[`gameplay_foliage.py`](../scripts/gameplay_foliage.py) regenerates them and refuses
unknown checksums, non-MIPS or non-ELF32 little-endian input, truncated headers and
out-of-range segments. Six focused rejection/address-resolution tests pass.

Neither Xbox nor PC ships these PS2 executable files. Whether corresponding
XBE/PE tables are identical is **NM**. `elf_for` still uses the original executable
when present. A non-PS2 source without that ELF now loads the validated numeric
artifact through `bake::foliage::numbers`, preserving the same EE arithmetic.
Focused checks compare the numeric table against both actual PS2 executables.
The PS2 source path retains its previous behavior, including its warning and
omission when the ELF is absent, to preserve existing PS2 bake bytes.

ELF-derived rules already transcribed into source are not missing input files:
BF1 animation fallback rows and bank order (`bake/src/anim_sets/bf1.rs`), BF2's
initial family/weapon registry (`anim_sets/bf2.rs`), and manifest animation action
names; hit/damage/movement/AI constants in `core`; material rules in `bake::alpha`;
and native menu/controller/HUD logic. They already travel with the program.
Changing their values by source platform would defeat the shared-gameplay rule.
This audit searched production bake/game/mungefs file reads and the pack/rebake
entry points; it does not claim to enumerate every historical RE reference.

## Other data dependencies

- **PS2 lighting lookup textures:** `CORE.LVL` holds `specular` and `fogspread`
  in both PS2 games, and `com_sfx_omniproj` in BF2. These exact names are absent
  from the corresponding Xbox and PC containers. `bake::spec` reads specular and
  omniproj; `game::viewer::scene` and the droideka renderer load the outputs.
  `bake/src/spec.rs` synthesizes those two textures when absent. That is a generated visual
  replacement, not measured pixel identity. The renderer already keeps a
  129-value numeric approximation of the PS2 fog curve in `shader.wgsl`, so
  `fogspread` is not an outstanding texture-file requirement at runtime.
- **Models, skeletons and collision:** map, soldier, weapon, vehicle, turret,
  droideka and destruction-chunk bakes read `modl`, `skel`, `coll`/`prim`, cloth
  and related assets. They derive hardpoints, hit volumes, enter/exit geometry,
  animation bindings and navigation/collision data. The PC readers make additional
  formats readable; they do not make shapes equal. Missing/changed
  shapes cannot be repaired by an ODF number. Use the canonical donor geometry,
  author replacement shapes and validate them, or refuse that content until a
  canonical source is available. Counts/bounds are not a substitute for shapes.
- **Terrain and worlds:** `bake::map`, `terrain_glb`, `mungefs::world`, `path`,
  `plan`, `terrain` and `water` consume heightfields, placed objects, boundaries,
  regions, hints, planning graphs, paths, collision layers, water and cutters.
  A table may carry spawn/mission values, names, scalar thresholds and placement
  transforms. A terrain heightfield or collision triangulation is geometry;
  serializing its vertices as JSON numbers does not make it an allowed gameplay
  table. A canonical donor, authored geometry or explicit content refusal is
  still necessary. The measurements, including which differences are on playable
  ground, are in [version-diff.md](version-diff.md).
- **Animations:** common/ingame and side banks (`zaa_`, `zaf_`, `sanm`/config and
  combo records) feed names, timing, root motion and skeletal clips. BF1 borrows
  BF2 throw clips and BF2 borrows BF1 prone clips. These are cross-game donor
  dependencies, not proof of platform exclusivity. Names/durations/events and
  scalar root-motion summaries fit a table; full animated skeleton tracks do
  not belong in the allowed names/numbers table. Matching duration alone cannot
  establish matching collision or weapon trajectories.
- **Classes and missions:** side/map ODF properties and mission script-derived
  values feed rosters, weapons, vehicles, ordnance, tickets, team counts, CPs and
  mission setup. The current runtime consumes baked RON and generated mission
  data, not Lua. A generated canonical PS2 property/mission table can remove
  platform-dependent numbers, provided scope, ordered repeated properties,
  defaults, inheritance and missing values survive. Shipping Lua bytecode is
  forbidden. Generic script constants are not automatically executed mission
  semantics; unsupported/dynamic mission branches must remain **NM**.
- **Audio:** `sounds`, `global_sound`, `beep`, `foley` and `shell` read sample
  banks, `snd_` inheritance in CORE, streams and cue/animation metadata. The
  PS2 voice/music ADPCM and container layout need codec support on each donor.
  PC audio decode and stream handling arrived with PR #94; Xbox audio decode and precise
  cue availability are **NM** here. Sound property numbers and cue IDs fit a
  table; actual samples/music do not. Missing audio requires a donor, a newly
  authored replacement, or an explicit disabled feature, not silent success.
- **Loading art, effects and UI models:** BF1 loading sheets/beep, BF2 loading
  widgets, side HUD weapon meshes, particle sheets/models, sky/water/detail art
  and sounds remain asset dependencies. `_dtl` names present only on PS2 often
  represent its split texture compression, while another version carries a
  whole painting under the base name. Do not label those as absent gameplay
  content or copy an incomplete base image as an equivalent texture.

## PC BF1: gameplay census measured; UI inventory remains open

The supplied Steam Classic 2004 BF1 install now passes the base-game census with 147 UCFB LVLs, 1,519 class
records and 44 mission records. Its AddOn folder is excluded before selecting the base LVL tree, so bundled Tatooine
3 content does not masquerade as retail base data. All PS2–PC and Xbox–PC BF1 gameplay census cells are measured in
[version-diff.md](version-diff.md). The raw source has material terrain, collision, class and sky-range differences;
reader support is not gameplay equality. The UI/fonts/localization/movie inventory above has not been repeated for
PC BF1, so those particular PC BF1 dependencies remain **NM**.

The maintainer's 2026-11-01 rule permits BF1 PC or Xbox without a BF1 PS2 disc only after the complete applicable
corrections exist. PC BF1 and PC BF2 together now build without a console disc (fingerprinted mission facts and PC shell replacements); a single PC game alone is still refused. On 2026-11-02 the maintainer approved publication of the existing correction tables, including absolute collision
coordinates, animation tracks, skeleton bones and height fills. Validation of source equivalence remains separate
from that publication approval.

The practical split is: canonical names/numbers can establish the gameplay
configuration; canonical shapes and animation behavior still require validated
inputs; textures/models/audio/fonts/movies remain donor or replacement assets.
The reference tables alone do not establish a validated Xbox- or PC-primary
build, and PC-only builds remain outside the product input policy. The options for a
build with no PS2 disc are in [version-diff.md](version-diff.md).
