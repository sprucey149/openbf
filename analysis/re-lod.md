# OPUS RE (2026-10-16) - [O] UNAUDITED — the LOD switch rule: which model the PS2 draws at which distance (card LOD)

Card LOD (lane 2). Question: the port draws LOD0 forever; when does the PS2 draw a `modl`'s other LODs (LOWD, `_low1`)?
Builds on `re-far-lowd.md` (UTAS: the far scene draws a LOWD) and corrects `re-softskin.md` §7 (`GeometryLowRes`).
Tools and frames: `engine/artifacts/lead_tmp/re/lod/` (`tools/`, `ps2/`, `port/`); census `tools/gmods.json`.

## 0. Answer

1. **There is no LOD chain to switch along.** A disc `gmod` holds ONE model in a size slot (the munger's choice by
   count: LOD0 ≥ 352, LOD1 ~250–330, LOD2 3–944) plus, often, a `LOWD`; of 951 BF1 / 1,784 BF2 geometry names only 2 per
   disc have two of LOD0/1/2 (§1). So the PS2 picks between **the model, its LOWD, and nothing** [V census].
2. **Far scene — the LOWD, per pixel, beyond NearSceneRange** [O, both games]: on a map with `FarSceneRange` > 0, every
   object whose gmod has a LOWD is drawn AS its LOWD when its sphere reaches past `NearSceneRange[1]·S` (b), culled at
   `FarSceneRange`; the composite shows it wherever the near scene's pixel is beyond b..c (§4). An object with no LOWD
   never enters the far scene, and the near scene's frustum ends at `NearSceneRange[2]·S` (c): **past c it is not drawn**
   (faded out b..c) (§4.3). Proved on the PS2 at two NAB1 stations: swapping the tree gmod's LOWD slot for its LOD0 turns
   the far crowns into the port's bare branches (39–43 levels, noise 0.4–2.2) and leaves a Gungan head at 50 m alone (§6.1).
3. **Near scene (to c)** [O]: per frame, per queue, the objects are popped in order and each takes its slot model if the
   frame's running count for that slot still fits the slot's BUDGET, else its LOWD, else nothing.
   - **BF1** (`FUN_004c1ef0`): nearest first; budgets (gmod queue) 35,000 / 20,000 / 20,000 for slots 0 / 1 / 2. No
     size or density term (§2).
   - **BF2** (`FUN_005ace40`): largest on screen first, key = (r·S/d)² (edge-clipped objects scaled down); an object
     under `rendering.minScreenSize` (5) is not drawn (key < 25/208000); **a LOWD-bearing gmod skips its model when
     count / key > 208000 · `rendering.maxVertexDensity` (1.0)**, i.e. it is drawn as its LOWD beyond
     **d = r·S·√(208000 / count)**; budgets 100,000 / 20,000 / 20,000 (§3). Proved on the PS2 by a threshold test: the
     LOD0 count poked to 0.6× / 1.6× of 208000·key flips exactly the instances the formula says (§6.3).
   - Objects whose record has flag 4 (a segment flag, `FUN_0057e610` / `FUN_004978a0`) go to the depth-sorted list and
     are drawn at lod 0 with no choice (both games, sort switch default 1) (§3.3).
4. **The budgets do not visibly bind at real poses** [measured]: budgets → ∞ changed nothing beyond the control's noise
   at four BF1 poses (two of them where a RAM replay of the chooser has the gmod queue at its budget) and two BF2 poses;
   budgets → 0 (the positive control) turns near trunks, floors and stairs into LOWD cards (§6.2, §6.4).
5. **Soldiers DO switch to `_low1`** [O] — `re-softskin.md` §7 was wrong (its search missed Ghidra's
   `0xffffffffa490eba1` form): `GeometryLowRes` builds a two-model record (hi, low) in the soldier queue. BF1: nearest
   first, the hi model while the frame's hi counts fit 10,000 (≈ 4 soldiers of 2,370); BF2: 2,000 per hi soldier of
   8,000 (4), and only while r·S/d ≥ 0.098 (the density test); everyone else is drawn with the `_low1` model (§5).
6. **The port** draws LOD0 everywhere (`GeomModel::best_model`), never a LOWD (but UTAS's UTA1 extension), never
   `_low1`, and on far-scene maps keeps drawing objects that have no LOWD past c.

## 1. The data — `tools/gmodcen.py` over every LVL of both discs [V measured]
| | gmod chunks | names | LOD0 | LOD2 | LOD0+LOWD | LOD1 | LOD2+LOWD | LOD1+LOWD | two of LOD0/1/2 |
|---|---|---|---|---|---|---|---|---|---|
| BF1 | 1,233 | 951 | 279 | 323 | 197 | 74 | 56 | 19 | 2 |
| BF2 | 2,989 | 1,784 | 714 | 490 | 318 | 122 | 101 | 37 | 2 |

Counts by slot (median): BF1 LOD0 706, LOD1 302, LOD2 108, LOWD 184; BF2 1,072 / 295 / 86 / 252. INFO's f32 is 1.0 on
every gmod (the mod tools' `-lodbias`, so no bias exists on the discs); INFO's u32 is 0 / 1 / 2 (stored at gmod+4, no
reader found [H]). The loader fills empty slots from each other (re-far-lowd §2.1), but the record's MASK keeps the
authored slots (§2), so "the model" is the one authored slot.

## 2. The LOD record and the BF1 near scene
- Record (BF2 `FUN_001b5000` 59950–60004, BF1 `FUN_001a91e0` 51375–51430): `rec[0..3]` = each slot model's +0xa8
  count; `rec[4]` mask 1/2/4 for LOD0/1/2 (the LOWD sets no bit); flags byte `rec+0x18` |3 iff a LOWD, |4 iff a model has a
  flagged segment; `rec[5]` = the queue (1 from both gmod loaders, BF1 54165); `rec[7]` = 1000.0. [O]
- Budgets, set once (BF1 `FUN_004790e0` 540524–540535 through `FUN_004c1cd0`, its only caller — 4 `jal` words in the
  ELF; BF2 `FUN_00563bb0` 698045–698056 through `FUN_005acc68`, its only caller — 4 `jal` words) [receipt:eed70a5e]
  [receipt:7405044f]; RAM in the NAB1 state = the ELF:

| queue | user | BF1 heap / slot 0 / 1 / 2 | BF2 heap / slot 0 / 1 / 2 / 3 |
|---|---|---|---|
| 0 | BF1 soldiers | 30 / 10,000 / 0 / 1,000 | 200 / 10,000 / 5,000 / 5,000 / 40,000 |
| 1 | gmods (both) | 500 / 35,000 / 20,000 / 20,000 | 500 / 100,000 / 20,000 / 20,000 / 20,000 |
| 2 | BF2 soldiers | 50 / 1,000 ×3 (drawn without choice) | 30 / 8,000 / 0 / 1,000 / 40,000 |
| 3 | — | 200 / 10,000 / 5,000 / 5,000 | 100 / 1,000 ×3 / 40,000 (drawn without choice) |

- BF1 traversal `FUN_004c5808` (near branch): depth < `rec[7]` (586896); key = view z − (|x|+|y|)/2 + r (586905–586907),
  so the heap pops the NEAREST first; flag-4 records go to the sorted list when `DAT_00522fd0` (ELF 1) (586898); a record
  with flag 1 but not 2 would be dropped past b·S when b < FarSceneRange (586884–586893) — no builder makes one.
- BF1 chooser `FUN_004c1ef0` (584113–584240): the first mask slot k with running[k] + count[k] ≤ budget[k] (584206–
  584216); none → lod 3 (the LOWD) if flags & 1, else not drawn (584218–584228); queue 2 is drawn without a choice, at
  most 30 a frame (counter 0x1d, 584143 / 584173), the rest dropped (584237). [O]
- BF1 near frustum: `FUN_004c6608` far = `FUN_00479da0()` (`DAT_00677664`, NearSceneRange[2]) × cam+0x90 (587431–587433).

## 3. The BF2 near scene
### 3.1 The key — `FUN_005af6b0` near branch [O]
key = (r·S)² / |p_view|² (746134–746143), S = cam+0x140 (the zoom) when `DAT_00746028`; an object the frustum test
found partly outside (obj+0x44 & 1, 746024–746026) is scaled down by its cut-off share at the near and side planes
(746145–746165); clamped to 100 and pushed into queue `rec[5]` (`FUN_005acd68` 743869–743870). A LOWD record (flags 3) is
queued at any distance inside the near frustum (746107–746110); the frustum ends at `FUN_00565130` (`DAT_00746020`) × S
(`FUN_005b06b8` 746692–746694).
### 3.2 The chooser — `FUN_005ace40` 743899–744045 [O]
Pops largest key first (743936–743962); key < `DAT_0060c858`² / 208000 → not drawn (743964); slot k (mask) is skipped
when (`showMissingLOD` or flags & 1) and `DAT_0060c854`·208000 < count[k] / key (743972–743977), else taken if it fits
the budget (743983); none → lod 3 if flags & 1, else not drawn (743991–743995). Console variables (`FUN_005ad308`
744048–744070, strings 0x655598…): `rendering.maxVertexDensity` = 1.0, `rendering.minScreenSize` = 5.0,
`rendering.soldiers` = 1 (0 skips queue 2, 743929), `rendering.showMissingLOD` = 0; ELF words 0x3f800000 / 0x40a00000
/ 1 / 0; no writer in the text, only the console registration [receipt:b54a78e3] (the console can set them).
ELF constants: 0x5ace54 `lui 0x36a1` (4.807692e-06), 0x5ad160 `lui 0x484b / ori 0x2000` (208000.0).
### 3.3 The sorted list [O]
A record with queue < 0, or flag 4 while `DAT_0060c8b0` (ELF 1), goes to the depth-sorted list (746130, 746174–746175),
drawn at lod 0 by `FUN_005b0ab8` (746873); BF1 `FUN_004c6928` the same (587599–587600). In the showcase states 1–2
LOWD-bearing gmods per map carry flag 4 (NAB1 `rep_fly_jedifighter`, TAT1 `tat1_bldg_moisturefarm`, MYG1
`myg1_bldg_central_building`, HOT1 `all_fly_snowspeeder`).

## 4. The far scene and what is not in it
4.1 Insertion (both): only flags & 1 records (BF2 746038, BF1 586920), when (d + r) − b·S ≥ 0 (BF2 746058–746060, BF1
586939–586941); BF2 also needs FarSceneRange > b·S (746063).
4.2 The walkers draw lod 3 = the LOWD, the opaque branch only past d − r > b·S (BF2 `FUN_005b0330` 746650–746659,
≤ 0x100 objects, soldier-queue records ≤ 10, a size test 746649; BF1 `FUN_004c62b0` 587389–587400, culled at
d ≥ FarSceneRange 587388, ≤ 200 objects 587313, queue-0 records ≤ 10 587345).
4.3 Composite (re-fog §2.5): near pixels nearer than b, a ramp b..c, the far target past c. So an object without a LOWD
fades out b..c and is gone past c; on a `FarSceneRange(0)` map (BF1 END1/RHN2/YAV1, BF2 YAV1/DAG1/NAB2, re-fog §6)
nothing is in the far scene. Scene ranges of the showcase maps (bake): NAB1 100/130/1000, TAT1 100/150/2000,
YAV2 100/150/350, UTA1 40/55/400, MYG1 50/80/300, HOT1 200/270/800.

## 5. Soldiers — `GeometryLowRes` [O]
- BF1 `FUN_0020f2e0` 123451–123461 / BF2 `FUN_00252bb0` 171132–171142: the low gmod at class+0x182c / +0x744, the
  class record at +0x180c / +0x724 = `FUN_001a9180` / `FUN_001b4f98`(hi = the GeometryName gmod's model — BF1
  *(class+0x13dc)+0x1c, BF2 *(class+0x160)+0x1c — low). The GeometryName branch builds the same record with no low
  model; a third class parser per game builds one too (B1 `FUN_001d3f68` 81692, B2 `FUN_001fd2f8` 112766) [D].
  ELF: the hash is built once per ELF (`tools/luiscan.py`: B1 0x20f788, B2 0x253438) [receipt:e26ec903]; no port code
  names it [receipt:83b3e468].
- Records: BF1 counts (hi +0xa8, −1, low +0xa8, low +0xa8), mask 5, queue 0, flags 1; BF2 (2000, −1, 2000, 2000), mask 5,
  queue 2, flags 1. With BF2's 1,000 slot-2 budget a 2,000 soldier never fits slot 2, so a soldier that is not hi is
  lod 3; BF1 fits ≈ 1 soldier in slot 2 (`_low1` 508), the rest lod 3.
- Draw: BF2 `FUN_00239bb0` lod 0 → the hi gmod, else the low one (153629–153645), unless `DAT_005ff0fa` (ELF 0, no
  writer) forces low; lod forced 0 by two viewport tests (153187–153190, 153212–153213; the second = the viewport's own
  soldier) and 1 by a camera-mode test (153196–153202) [H what the first and third test]. BF1 `FUN_002013a0`: lod ≠ 0
  with a low gmod → the low model (113447–113449, 113712–113713); lod forced 1 under a camera test (113186–113190)
  [H what it tests].
- 138 of 140 soldier / hero classes with their own `GeometryName` carry a `GeometryLowRes` (bake classes; the two
  without are the droidekas). RAM replay (`tools/budget1.py`, 120 BF1 states): more than 4 soldiers in the near scene in
  13 states (the 5th onward low).

## 6. PS2 checks (live data pokes, two grabs per poke, a control grab after the restore)
6.1 **BF1 NAB1 two stations** (`ps2/lod_walk1.py`; `ps2/bf1_nab1/{A,B}_b000_*`, strips `*_treeline.png`): A = the
LEAF spawn eye, B = 50 m from a Gungan head. Region mean |Δ| vs the base:

| region | treelowd | alllowd | budget ∞ | budget 0 | control |
|---|---|---|---|---|---|
| A treeline left / right (262–555 m trees) | 42.1 / 42.7 | 42.1 / 42.8 | 2.3 / 0.5 | 2.4 / 0.4 | 1.5 / 0.4 |
| B far tree | 39.3 | 39.4 | 2.0 | 2.7 | 2.2 |
| B Gungan head 50 m (near scene) | 2.8 | 3.1 | 3.1 | 4.7 | 2.8 |

6.2 **BF1 BES1 / END1** (the replay's two most loaded poses): background mean |Δ| budget ∞ 2.41 / 1.99 vs control 3.17 /
2.17; budget 0 15.65 / 19.15 (near trunks and a log become flat LOWD cards, `bf1_end1/W_b000_sheet.png`).
6.3 **BF2 MYG1 threshold test** (`ps2/run_myg1t.sh`, `bf2/myg1_cp2cp1_*`): `small_spiral` count 1134 → 6402 (0.6 ×
208000·key) / 17405 (1.6 ×); `stump_spiral` 628 → 23712 / 63232. Mean |Δ| vs the control: towers 30 m: lo 0.7 / 0.9, hi
20.9 / 16.0, dens 0 21.5 / 16.2, stump pokes 0.7–1.0; stump 5 m: its lo 0.6, its hi 22.9, dens 0 23.1, tower pokes 0.8.
6.4 **BF2 HOT1 spawn, MYG1 cp3→cp7** (`bf2/hot1_quad.png`, `myg1_quad.png`): `maxVertexDensity` 0 turns the near
LOD0 objects into their LOWDs (HOT1 trench band 47.4 vs 10.5; MYG1 floor 34.3, stairs 19.5 vs 0.9 / 1.7); budget ∞ no
change (10.6 vs 10.5; 1.7 / 2.1); 1000 no visible change on the small objects predicted LOWD (14 px; walkers move).

## 7. For the port (build spec; card LOD → phase build)
1. **Follow** the far-scene rule: on a map with FarSceneRange > 0, a LOWD-bearing instance's fragments past c are its
   LOWD, nearer than b its model, a b..c crossfade, the LOWD culled at FarSceneRange. It subsumes UTAS's
   `bake::far_lowd` extension (the UTA1 ring's far half is the LOWD past c).
2. **Named deviations (not followed, modern):** the budgets (measured immaterial); BF2's 5-px size cull (480-line);
   BF2's near-scene density switch (a vertex-rate rule) — a knob, default off; non-LOWD objects past c (draw distance,
   kept drawing — STATUS question); soldier `_low1` (a 4-hi-res budget).
3. Acceptance: `lead_tmp/re/lod/tools/accept.py` — the dark crown share of the NAB1 station-A treeline boxes, PS2
   57.8 / 57.4 %, the PS2 with its LOWD knocked to LOD0 23.0 / 13.2 %, the port at f7f19c6 18.5 / 15.0 % (bar ≥ 45 %;
   port frame `lead_tmp/re/lod/port/nab1_A_ps2_ps2lowd0_port.png`); a unit test of the per-fragment pick; the goldens
   declared. The port's b..c crossfade: one dithered threshold t ∈ [b, c] per pixel read by both draws (model where
   d < t, LOWD where d ≥ t), so the two never overlap; the shader already has per-instance `cull` / `cull_near`.

## 8. Checks run (full output `engine/artifacts/lead_tmp/re/lod/checks.txt`)
```
$ re_check fn b2 743964 743976 743983 743992 743999 743929 744034 -> FUN_005ace40 · 698046 698049 -> FUN_00563bb0
  746024 746038 746058 746063 746108 746124 746130 746134 746141 746143 746147 746168 746175 -> FUN_005af6b0
  743870 -> FUN_005acd68 · 746694 -> FUN_005b06b8 · 746873 -> FUN_005b0ab8 · 171132 -> FUN_00252bb0 · 59929 -> FUN_001b4f98
  153187 153213 153629 153645 -> FUN_00239bb0 · 149779 -> FUN_00234a68 · 744058 -> FUN_005ad308
$ re_check fn b1 584143 584209 584216 584221 584228 -> FUN_004c1ef0 · 540526 540529 -> FUN_004790e0
  586884 586896 586898 586905 586907 586920 586939 -> FUN_004c5808 · 587313 587345 587388 587389 587399 -> FUN_004c62b0
  587433 587466 587469 -> FUN_004c6608 · 587599 -> FUN_004c6928 · 123451 123455 -> FUN_0020f2e0 · 51357 -> FUN_001a9180
  113189 113449 113712 -> FUN_002013a0 · 54165 -> FUN_001ace78 · 81692 -> FUN_001d3f68
$ re_check word b2 0x60c854 2 --f32 -> 1.0, 5.0 · 0x60c85c -> 0x00000001 · 0x60c8b0 -> 1 · b1 0x522fd0 -> 1
$ re_check word b2 0x5ff0f8 -> 0x00000100 (byte 0x5ff0fa = 0)
$ re_check word b2 0x655598/0x6555b8/0x6555d0/0x6555e8 --str -> rendering.maxVertexDensity / minScreenSize / soldiers /
  showMissingLOD
$ re_check writers b2 DAT_0060c854 DAT_0060c858 DAT_0060c85c DAT_0060c85d DAT_0060c8b0 DAT_005ff0fa; b1 DAT_00522fd0 -> 0 each
$ re_check callers b2 FUN_005acc68 -> FUN_00563bb0 x4 · b1 FUN_004c1cd0 -> FUN_004790e0 x4
$ re_check absent b2 0x0c16b31a 0x005acc68 --elf -> jal x4 in the BF2 ELF, no data word [receipt:eed70a5e]
$ re_check absent b1 0x0c130734 0x004c1cd0 --elf -> jal x4 in the BF1 ELF, no data word [receipt:7405044f]
$ re_check absent b2 a490eba1 -> B1 FUN_0020f2e0 123451, B2 FUN_00252bb0 171132 [receipt:e26ec903]
$ luiscan.py 0xa490eba1 -> b1 1 build 0x20f788/ori, b2 1 build 0x253438/ori (control 0x47c86b4a GeometryName: 19 / 25)
$ eedis2 bf2 0x563bf4..0x563c4c: addiu t0 1000, ori t1 0x9c40, a1 30, a2 8000 ... lui a2 1 / ori 0x86a0, 20000 x3, a1 500
$ eedis2 bf1 0x479148..: a2 10000, a1 30 ... ori a2 0x88b8 (35000), 20000 x2, a1 500
$ eedis2 bf2 0x5ace54 lui at 0x36a1 (4.807692e-06) · 0x5ad160 lui at 0x484b / ori 0x2000 (208000.0)
```
PS2 runs: `ps2/run_nab1.log` (lod_walk1.py NAB1 A/B), `run_bes1.log`, `run_end1.log`, `run_hot1.log`, `run_myg1.log`,
`run_myg1t.log`; RAM replays `tools/predict2.py` (BF2), `tools/budget1.py` (BF1, `budget1_bf1.txt`, 120 states).

## 9. Still [H]
- INFO's u32 (0 / 1 / 2 — the mod tools' `-lodgroup`?) is stored at gmod+4; no reader found (not searched by offset).
- What the camera / viewport tests that force a soldier's lod read (BF1 113186, BF2 153187 / 153196).
- The RAM replays approximate the frustum (a sphere-in-cone test); their overflow counts are an upper bound — the PS2's
  budget-∞ frames are the evidence that budgets do not show.
- BF2 NAB2 (no far scene) PXWB F61's far window blocks: possibly the BF2 near density switch (the houses' LOWD inside
  the 100–150 m near scene) — not checked.
- `_low1` vs hi-res on screen was not imaged on the PS2 (the rule is read, not knocked out).

## 10. Verifier (Workflow `lod-re-verify`, 5 agents at effort medium, claims + citations only; 2026-10-16)
26 claims: batch A 8/8 SUPPORTED; batch B 5 SUPPORTED, 2 PARTLY (B3: BF1 queue 2 capped at 30 a frame; B4: the flag-1
cull only when b < FarSceneRange — both folded into §2); batch C 3 SUPPORTED, 1 PARTLY (C3: a second lod-0 forcing and
the `DAT_005ff0fa` low forcing — folded into §5). The 7 absence / twin claims (budget setters' only callers, the flags'
writers, `GeometryLowRes` built once per ELF and re-softskin §7 wrong, the chooser / traversal / soldier-record twins)
went to two separate verifiers: both SUPPORTED all 7, none overturned. Extras taken: BF2's walker caps (§4.2), the
third record-builder callers (§5), BF2's hi model offset (§5).

## 11. Soldier implementation re-check (2026-11-01, issue #69)

Re-read the soldier record builders and both choosers against the local PS2
executables/decompilation. BF1 `0x001a9180` takes the high model's `+0xa8`
count; `0x004790e0` gives queue 0 a 10,000 high budget. The count is the
model's total strip indices, **not** decoded vertices (see re-softskin §4).
BF2 `0x001b4f98` charges 2,000 per soldier; `0x00563bb0` gives queue 2
8,000. `0x005ace40` applies the 208,000 density divisor. Direct ELF reads at
`0x0060c854` and `0x0060c858` still give 1.0 and 5.0. Far walkers
`0x004c62b0` / `0x005b0330` cap the soldier queue at ten; BF2 also tests
`radius² / (view_depth + radius)² >= 25 / 208000` before drawing the low mesh.

The pre-change port has **no soldier distance cull**: `SoldierVisuals::posed`
selects lifecycle/cloak/seat eligibility and submits the full body; the renderer
expressly excludes skinned draws from its world AABB cull. Soldiers therefore
remain full detail beyond the map's near range, until the projection far plane
clips them. This corrects the issue's uncertainty about the existing cull.

The new bake takes `GeometryLowRes` through the class inheritance chain and
writes a `<high>.low.glb` companion, with joint indices and bind positions
normalized to the high skeleton. The renderer reuses the high animator and
palette. `<high>.lod.ron` carries the authored sphere and strip count. No model
loads, name lookups, bone remapping or fresh selection-scratch allocation occur
per frame after warm-up. Missing/incompatible companions preserve the previous
unlimited full-detail submission and require a targeted soldier rebake.

Saber-bearing classes retain their existing full-detail submission alongside
their independent blade/hilt/trail passes; this slice leaves that hero rendering
scope unchanged. Specialized droidekas already use a separate renderer.

Both visual tiers use the PS2 budgets for ordinary soldiers; no larger default budget is inferred from
hardware names. Selection is per camera, with the viewport's own soldier kept
high. BF1 uses its depth/off-axis heap key; BF2 uses projected sphere size,
including side/near clipping attenuation. Far candidates begin at
`depth + radius >= NearSceneRange[1] * zoom`; at most ten nearest candidates
are eligible (both native heaps rank `depth - 3 * radius`), and `depth < FarSceneRange` is required. The far range of zero
has no far layer. Near bodies remain eligible while
`depth - radius <= NearSceneRange[2] * zoom`.

**Remaining difference:** this implementation selects whole bodies in the
near/far transition. It does not add the world renderer's per-fragment b..c
dither to the skinned pipeline. The far queue is still charged for the bodies
in that transition, so it cannot draw an additional ten beyond those already
counted. Saber-bearing classes stay outside this rule (above).

## 12. Soldier picks against PS2 save states (2026-11-01, issue #69)

Sixteen existing save states (ten BF1, six BF2; crowded spawns) give each state's own
soldier queue from RAM: sphere, strip count, record flags, the camera, and the map's
scene ranges. No new capture was taken. Run through the port's chooser
(`ps2_state_queues_through_the_chooser`), **392 of 393 bodies** agree with a separate
replay of the PS2 rules on the same rows (BF1 from the `FUN_004c1ef0` replay, BF2 from
the §5 rule plus its 8,000 budget). The one difference is a body clipped at the frustum,
whose key the port attenuates and the replay does not.

That check found two errors the unit tests could not:
- **BF1's heap key had both signs wrong.** It preferred off-axis and smaller bodies. The
  native key, `FUN_004c5808`, is `-depth - off_axis/2 + radius`, largest first.
- **The draw sphere is 1.0 m past the model's authored one.** RAM against the bake, matched by
  strip count, is exactly +1.000 on all 15 BF1 classes with a unique count. BF2 charges a flat
  2,000, so there the match is by radius: 9 of 11 distinct RAM radii are a baked radius plus 1.0.
  Five BF1 counts and two BF2 radii match no baked class and were not traced. At BF2's rules the
  margin moves a full-detail body from about 12 m to about 23 m and the size cull from about 114 m
  to about 205 m.

What the state frames can and cannot show: the frames fit the picks where a body is on open
ground (BF2 Yavin 1's landing zone has no soldier at the bodies picked hidden past its 100 m
range with FarSceneRange 0). At these distances a low body is too few pixels to tell from a
high one by eye, so the frames do not separately prove which mesh is on screen; the mesh choice
rests on the RAM picks.

## 13. What the low models cost and save (2026-11-01, issue #69)

`--bench-toggle soldier-lod`: the same frozen battle, full detail against the PS2 rule in
alternating blocks, release build at 1280x720, three runs on each GPU, medians. A BF2 map where
the shadow switch does nothing is the noise control (`re-vehicle-shadow.md` §9): within 0.02 ms
on the RTX, and -0.33 to 0 ms on the Intel. Frame is whole-frame wall clock; list is the CPU
building the frame's draw lists.

| map, bots, view | GPU | skinned draws | frame change | list ms |
|---|---|---|---|---|
| BF1 NAB1, 120, overview | RTX 4000 Ada | 557 → 21 | -1.7 ms | 1.30 → 0.54 |
| | Intel RPL-S | | -3.9 ms | 1.35 → 0.65 |
| BF1 NAB1, 120, player eye | RTX 4000 Ada | 557 → 1 | -1.6 ms | 1.32 → 0.54 |
| | Intel RPL-S | | -3.6 ms | 1.32 → 0.55 |
| BF2 NAB2, 120, player eye | RTX 4000 Ada | 558 → 45 | -1.2 ms | 0.90 → 0.39 |
| | Intel RPL-S | | -4.5 ms | 1.42 → 0.74 |
| BF1 NAB1, 16, overview | RTX 4000 Ada | 66 → 24 | -0.13 ms | 0.37 → 0.29 |
| | Intel RPL-S | | -0.45 ms | 0.41 → 0.34 |
| BF2 NAB2, 16, player eye | RTX 4000 Ada | 69 → 6 | -0.28 ms | 0.16 → 0.09 |
| | Intel RPL-S | | -0.57 ms | 0.23 → 0.14 |

Most of the gain is hidden bodies rather than cheaper ones: past the near scene the rule draws
at most ten. Those numbers do not include a window's presentation or vsync. No larger default
budget was chosen: at the PS2's budget the gain above is already free, and every build should
show the same bodies.
