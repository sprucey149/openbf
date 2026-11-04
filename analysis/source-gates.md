# Which inputs a build takes, and what the others still need

Measured 2026-11-02 on the tables in `manifest/data/gameplay/corrections-v1/`. Update it when a switch in `scripts/input_scan.py`
changes or a map is run.

## What is accepted

Both `XBOX_SUPPORTED` and `PC_PRIMARY_SUPPORTED` are true. An Xbox disc can be either game's only source, and a PC BF1 install
can be BF1's. PC BF1 plus PC BF2 can also supply both games without a console disc. The scanner records an explicit PC-only
plan: canonical mission tables, PC BF2 shared UI and shell sound, and both copies for the borrowed animations. Single-game
PC builds remain refused with "both games needed". Beside a PS2 disc, the PS2 still supplies the gameplay and the others
offer larger textures. PC BF2 does contain the BF2 HUD, fonts and most shell sheets; its missing backdrop uses original
project artwork and its fingerprinted galaxy movies replace the console ship loops, keeping the same screen structure.

The PC Citadel snowdrift and Mustafar console models themselves reference sheets absent from both PC installs.
The PC-primary bake makes two explicit appearance substitutions, gated by decoded pixel fingerprints: Citadel uses its
own `rhn2_main_1` snow sheet for `rhn_bldg_snow_2`; Mustafar uses rectangle (64,112,192,160) of its own 512-square
`tan4_prop_console` blue display for `mus1_prop_console_fx`. These are local artwork replacements, not same-art proofs.
An exact sheet takes precedence and changed donor pixels do not receive the substitution. Jabba's axe instead resolves
the exact `gam_1st_weap_inf_axe` sheet from the same copy's SIDE/GAM LVL. No console artwork is borrowed. Placement,
collision, navigation and mission rules do not change. Existing PC BF1 packages also gain the snow sheet and the
canonical turret stop cue; those are intentional presentation corrections, rather than byte-identical output claims.

Nothing outside the public code and the player's own files is read. Each correction row is an instruction list over the donor
file, and the bake refuses a file whose fingerprint or rebuilt result differs from the table's, so `rebake.sh` needs no gameplay
check of its own: a drifted file stops the bake.

## Recovered structures and map availability

BF1's Cloud City and Rhen Var maps are now included from every accepted BF1 source. A full inventory of the 142 Xbox BF1 and
147 PC BF1 LVLs found that those versions assemble several buildings differently from the PS2. The named PS2 pieces are absent,
but their larger source structures exist. Exact resources from another LVL are pulled first: both versions have Rhen Var's broken
corner in RHN1 rather than RHN2, and PC has a basebrick in RHN2 rather than RHN1.

The remaining pieces use lower-detail stand-ins built locally from the corrected collision shapes, with materials and texture
coordinates projected from that copy's placed artwork. The new surfaces use the map's lighting instead of unrelated baked shadow
colors from the donor's old surface. No render mesh or texture is included in the repository.
`render-bounds-v1.json` contains only measured numeric crop envelopes; `render_recovery_bounds.py` reproduces them from the player's
reference files. Related retained composites are cropped to their corrected PS2 visibility boxes so they do not draw the removed
pieces again. The smaller pieces can look coarser: some domed caps become simple cylinders, and projected textures can stretch.

Three recovered pieces need both their mesh and independent live primitives: Cloud City's chamber and chamber shaft, and
Rhen Var Harbor's `rhn1_bldg_fortress_05`. Recovery verifies that their owning classes have no collision-layer overrides and
their primitive masks are zero before retaining that union. Eight measured stand-in types also deform generated boundary
vertices into the numeric art envelope instead of clipping whole wall faces away. Degenerate and exact duplicate triangles
are removed on those eight corrected pieces; displaced surfaces are retained. These instructions change only locally generated
replacement artwork, never the collision or an existing donor render model.

Map availability does not establish complete artwork/collision alignment. The recovered structures resolve to models and keep
PS2 collision and navigation unchanged, but existing donor models still have measured surface discrepancies. The
broader automated checks below do not replace live play or a running-original comparison.

All 34 ground maps are now included from the accepted sources. The six experimental space maps require explicit
opt-in. Xbox BF2 Jabba's Palace uses a local textured stand-in for
`tat3_prop_cz1droid_parts`, confirmed absent from all 185 disc LVLs including the sound banks. Its donor-only water cell
is removed by a fingerprinted correction; four complete rounds matched every tick across two seeds and both eras.
The counts below record the original unresolved placements, before shared-resource lookup and recovery.

| Source | Maps needing recovery | Originally unresolved instances |
|---|---|---|
| Xbox BF1 | bf1_rhn1, bf1_rhn2 | 39, 6 |
| Xbox BF2 | bf2_tat3 | 1 |
| PC BF1 | bf1_bes2, bf1_rhn1, bf1_rhn2 | 58, 93, 12 |

PC BF1 Kashyyyk Islands keeps its 16 `kas1_mist` instances without a model: going by the name they are mist, not
solid, which has not been looked at.

## Bytes per source

| Source | Table | Instructions |
|---|---:|---:|
| bf1-pc | 1,938,038 | 515,828 |
| bf1-xbox | 1,332,492 | 317,793 |
| bf2-xbox | 1,060,608 | 407,605 |
| bf2-pc | 1,899,665 | 585,269 |

The table includes names, classes, world records, height rows and height fills. These tables replace 2.6 MB of unique private
collision plus navigation, skeleton and animation files, which no build reads any more.

## Per map

"Heights" counts height rows one cell past the line at most. "Fills" counts rectangles of ground the 512 m aim ray can read beyond that.
"Instructions" is the bytes of that map's own lists; shared side and common lists are extra. "Round" records the
earlier comparison of every tick of one full round
with the PS2-source build: seed 42, Normal, Conquest, default era. The wider checks below are separate. "No model"
records original unresolved instances before shared-resource lookup and recovery; it is not the current count.

| BF1 map | Xbox heights | fills | instructions | round | no model | PC heights | fills | instructions | round | no model |
|---|---:|---:|---:|---|---:|---:|---:|---:|---|---:|
| bf1_bes1 | 0 | 0 | 66,458 | match, 25,661 ticks |  | 0 | 0 | 76,639 | match, 25,661 ticks |  |
| bf1_bes2 | 0 | 0 | 4,259 | match, 14,750 ticks |  | 128 | 5 | 64,249 | match, 14,750 ticks | 58 |
| bf1_end1 | 259 | 1703 | 41,034 | match, 15,578 ticks |  | 264 | 1703 | 3,926 | match, 15,578 ticks |  |
| bf1_geo1 | 0 | 0 | 39,579 | match, 18,880 ticks |  | 0 | 0 | 61,270 | match, 18,880 ticks |  |
| bf1_hot1 | 194 | 5378 | 12,293 | match, 15,962 ticks |  | 272 | 5378 | 15,472 | match, 15,962 ticks |  |
| bf1_kam1 | 0 | 0 | 0 | match, 11,534 ticks |  | 0 | 0 | 0 | match, 11,534 ticks |  |
| bf1_kas1 | 75 | 0 | 881 | match, 13,969 ticks |  | 75 | 0 | 1,850 | match, 13,969 ticks | 16 |
| bf1_kas2 | 351 | 0 | 1,613 | match, 11,803 ticks |  | 353 | 0 | 3,592 | match, 11,803 ticks |  |
| bf1_nab1 | 72 | 0 | 3,077 | match, 14,688 ticks |  | 72 | 0 | 3,610 | match, 14,688 ticks |  |
| bf1_nab2 | 221 | 29 | 3,810 | match, 13,635 ticks |  | 221 | 29 | 7,136 | match, 13,635 ticks |  |
| bf1_rhn1 | 10 | 57 | 58,129 | match, 14,191 ticks | 39 | 172 | 83 | 77,315 | match, 14,191 ticks | 93 |
| bf1_rhn2 | 11 | 0 | 28,982 | match, 10,973 ticks | 6 | 52 | 94 | 36,781 | match, 10,973 ticks | 12 |
| bf1_tat2 | 0 | 14 | 5,332 | match, 14,227 ticks |  | 0 | 14 | 28,192 | match, 14,227 ticks |  |
| bf1_yav1 | 76 | 853 | 4,160 | match, 11,546 ticks |  | 76 | 853 | 4,200 | match, 11,546 ticks |  |
| bf1_yav2 | 0 | 0 | 1,839 | match, 12,456 ticks |  | 0 | 0 | 1,839 | match, 12,456 ticks |  |
| tat1 | 31 | 0 | 4,071 | match, 14,961 ticks |  | 31 | 0 | 7,831 | match, 14,961 ticks |  |

| BF2 map | Xbox heights | fills | instructions | round | no model |
|---|---:|---:|---:|---|---:|
| bf2_cor1 | 0 | 0 | 657 | match, 13,628 ticks |  |
| bf2_dag1 | 0 | 0 | 0 | match, 10,502 ticks |  |
| bf2_dea1 | 0 | 0 | 50,750 | match, 13,501 ticks |  |
| bf2_end1 | 0 | 0 | 45,118 | match, 14,547 ticks |  |
| bf2_fel1 | 0 | 0 | 6,304 | match, 12,453 ticks |  |
| bf2_geo1 | 0 | 0 | 1,139 | match, 12,156 ticks |  |
| bf2_hot1 | 0 | 0 | 25,276 | match, 9,939 ticks |  |
| bf2_kam1 | 0 | 0 | 555 | match, 13,294 ticks |  |
| bf2_kas2 | 0 | 0 | 12,774 | match, 12,356 ticks |  |
| bf2_mus1 | 0 | 0 | 89 | match, 11,198 ticks |  |
| bf2_myg1 | 0 | 0 | 0 | match, 13,634 ticks |  |
| bf2_nab2 | 0 | 0 | 0 | match, 12,374 ticks |  |
| bf2_pol1 | 0 | 0 | 1,593 | match, 13,708 ticks |  |
| bf2_tan1 | 0 | 0 | 1,034 | match, 11,899 ticks |  |
| bf2_tat3 | 0 | 0 | 117,151 | match, 10,937 ticks | 1 |
| bf2_uta1 | 0 | 0 | 19,477 | match, 14,171 ticks |  |
| bf2_yav1 | 76 | 853 | 3,210 | match, 12,651 ticks |  |
| tat2 | 0 | 0 | 21,382 | match, 12,208 ticks |  |

## Jabba source-water correction

Xbox BF2 Jabba's Palace stores a single water cell at Y = -10.5 in its terrain stub; the PS2 stub has no water.
The collision and navigation bake files matched, but that extra water removed 21 tactical positions before the round began.
At tick 1091 unit 0 first chose a different position, producing the first hash mismatch. Removing only the donor water restored
all 1,520 tactical positions and that tick's hash. A fingerprinted `remove_water` locator now strips that donor-only WATR before
any bake reader consumes it, without changing the PS2 path. The generator handles dry terrain stubs as well as heightfields.

With a fresh Xbox Jabba map bake and the retained source-specific common/side assets and missions, every tick matched the PS2-source
build through four complete Normal Conquest rounds: Galactic Civil War seeds 42 and 7 (10,937 and 10,280 ticks), and Clone Wars seeds
42 and 7 (11,282 and 10,284 ticks). These are automated source comparisons, not a comparison with a running PS2 or a live play check.
The separate droid-parts render prop is now recovered with a lower-detail local textured stand-in.

## Jabba render-piece reassembly

Xbox BF2 splits the torture room and its pipes across extra props whose original placements exactly coincide with the
unsuffixed models. Canonical world replacement removed those placements although their artwork was present in the LVL.
The bake now rejoins those verified source segments into the two canonical render models, rejecting changed bindings,
missing pieces or different placements. Collision and simulation placements are unchanged. Each segment keeps the lighting
program evaluated in its original model context: a naive merge changes BF2's model-wide prelit decision.

A 256-sample collision-to-render check over the Conquest scene reduced the maximum excess distance above the PS2 reference
from 3.93 m to 0.00065 m for the room and from 2.32 m to 0.0361 m for the pipes. The grate's missing faces live in the room
piece on Xbox: its per-object mismatch remains, while the scene-union excess falls from 2.81 m to 0.00025 m. These are sampled
measurements, not a continuous surface proof. Matching headless room views were inspected on PS2, Xbox and the PC BF1 plus
Xbox BF2 input set; the restored floor, ceiling and wall have visible textures. A controlled PS2 bake stayed byte-identical,
and the Xbox bake changed only the two reassembled model GLBs. Other maps and modes still need their own alignment checks.

## Still open

- **Unexercised simulation inputs.** Fresh independent PS2, Xbox and PC BF1/Xbox BF2 builds matched full simulation
  state hashes for 1,800 ticks in each of 67 authored missions at seeds 7 and 103: 402 source/scenario runs, covering
  Conquest and experimental Space Assault, and both eras where authored. Their logs contain no class, vehicle-pad or
  common-asset fallback. These bounded traces do not establish complete round endings or equality of inputs they never
  exercise. Class definitions and world identities still differ in fields the traces did not read, so no source is
  declared multiplayer-equal.
- **Space maps.** Space stays behind `OPENBF_BAKE_SPACE=1` and `OPENBF_SPACE=1`. On Xbox BF2 all six bake and match
  the PS2 at every one of 36,000 ticks on one seed, but no round reached a winner in that time, so an ending is not
  compared.
- **The look.** A placed-object gate compares corrected collision with placed render bounds and sampled triangle
  surfaces, including the PS2 artwork residual as a control. Fresh all-map checks still fail on existing donor-art
  discrepancies and unmeasured inputs. Headless captures of the recovered maps have been viewed; projected stand-in
  textures and some shapes remain coarse. No live game window was checked.
- **Non-root animation.** Other bones keep the donor's pose. Rounds match, but a pose read in a case those rounds did
  not reach would differ.
- **PC BF1 side models.** `rep_fly_vtrans` and `all_fly_kwing` have no model on PC; no BF1 mission or map uses them.
  Four speederbike wreck chunks are also missing there, so that wreck shows fewer pieces.
- **Foliage counts.** Six maps' grass counts differ from the PS2's on non-PS2 sources, which the build reports as a warning. Rounds match.
