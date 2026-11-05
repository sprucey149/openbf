# scripts

Every command runs from the repository root unless its header says otherwise. Each script's header has its usage.
Scripts stay in this flat folder because the notes under `analysis/` and the crate map cite them by path.

## Build a game from your own discs

| script | what it does |
|---|---|
| `pack.py` | what you own in `iso/` to a self-contained game |
| `input_scan.py`, `console_input.py`, `dlc_inputs.py` | recognize inputs by content, and say what a build makes of them |
| `rebake.sh`, `bake_dlc.py` | bake assets, and the two optional DLC heroes (`DLC.md`) |
| `extract_lua.py`, `gen_missions.py`, `gameplay_missions.py` | mission files from the mod tools' Lua |
| `scratch_bake.sh`, `cmp_bake.sh` | bake into a scratch tree, and compare it byte for byte |
| `world_fingerprint.py` | hash baked gameplay inputs, independently of texture and asset resolution |

## Same game on every source

| script | what it does |
|---|---|
| `gameplay_table.py`, `gameplay_foliage.py`, `gameplay_corrections.py`, `gameplay_programs.py` | export measured PS2 input values and generate the shared gameplay corrections applied to a player's own files |
| `version_census.py`, `version_geometry.py`, `version_collision.py`, `version_heights.py`, `version_sky.py`, `version_terrain_play.py` | measure how editions differ: names and numbers, never assets |
| `version_report.py`, `version_parity_check.py` | the measurement receipt, and seeded rounds across source combinations |
| `render_collision_check.py`, `render_scene_surface.py`, `render_model_dependencies.py`, `render_gpu_bounds.py` | require continuous surface certificates for every placed model, LOWD and far extension against PS2 artwork, charging world draw rounding for every placement; unresolved coverage, opacity, motion or LOD correspondence fails; `gates.py --groups alignment` |
| `render_contact_check.py`, `render_contact_proxy.py`, `render_contact_groups.py`, `render_query_frame.py` | alternative complete contact-surface proof within 1 cm, with identical active soldier/ordnance domains, complete primitive proxies, outward query-frame bounds and every matching placement; asymmetric or unsupported domains fail unresolved |
| `render_terrain_check.py`, `render_attachment_check.py` | the alignment gate's required terrain and moving-object receipts; absent or unmeasured inputs fail |
| `render_alignment_probe.py`, `alignment_known.py` | sampled ordinary-float PS2-residual comparisons and located open offenders; known entries remain measurable, and in-boundary samples do not establish whole-map reachability or acceptance |
| `render_alignment_sweep.py` | every placed object, terrain and side vehicle of every map, composed with neighbours and terrain against PS2's own residual and ranked; `--gate` measures five maps, including Citadel and Felucia, with tighter repaired-near-model limits; unlisted in-boundary excess above 0.25 m and unlisted unmeasured placements fail, and bounded outstanding rows allow 5 mm growth on the two added maps (5 cm on the original three); a sampled regression check, not acceptance |
| `render_input_stability.py` | record every GLB, RON, KTX2 and JSON input, optional sidecar presence and measuring source before and after; changes, missing files or a stale receipt fail |
| `data/` | the measurement receipt and the UI dependency list |

| `render_class_inputs.py`, `render_identity_baseline.py`, `render_recovery_bounds.py` | track placement inputs, identity baselines and conservative recovered surface bounds |
| `render_skin_support.py`, `render_terrain_dependencies.py` | verify moving skin support and terrain proof dependencies |

## Checks

The default build checks include `alignment`. Automatic checks require it when
bake, simulation, manifest or rendering code changes, and landing retains that
requirement. Give `gates.py` independent bakes using `--alignment-reference` and
repeat `--alignment-case NAME=ASSETS` for every source being accepted. Standalone
gate commands can use `OPENBF_ALIGNMENT_REFERENCE` and shell-quoted
`OPENBF_ALIGNMENT_CASES` with the same `NAME=ASSETS` entries. Missing roots fail.
Keep the report outside those input folders. The gate checks their content and
file metadata before and after measurement, and checks the current folders and
measuring code when validating a receipt. This verifies the supplied artifacts;
their extraction and bake provenance remain separate checks. Explicit `--map`
selections are diagnostic and cannot return alignment acceptance.
A scoped diagnostic, a source-only CI run, or a successful pack scene report is
not a complete alignment acceptance result.

| script | what it does |
|---|---|
| `gates.py`, `gates_selftest.py`, `gates_history.py` | run the gates and compare them with the stored values; test the shortcuts; replay a golden skip rule over history |
| `sim_gates.sh`, `gpu_tests.sh`, `goldens.sh` | sim referees, GPU referees, golden pictures |
| `gpu_tests.sh --render`, `--game FILTER`, `--game-lib [CARGO_ARGS]`, `--ui-sharp`, `--hud-item-layer` | headless serial renderer, ignored game, complete game-library and native HUD checks; all GPU test runs use `--test-threads=1` |
| `fdiff.py` | first divergence between two sim traces |
| `parity.py` | score the port against stored PS2 frames |
| `sound_bake_check.py`, `audition.sh`, `flythrough.sh` | check baked sounds; listen to a bank; one orbit video per map |
| `size_report.sh`, `size_waivers.txt`, `check_provisional.py` | file sizes and the crate map; provisional values must name their measurement |
| `tools/pure_move_check.py` | check a source split against a Git revision; retain the extraction proof and check rustfmt-only wrapping separately |
| `tests/` | `python3 -m unittest discover -s scripts/tests` |

## Measuring play

| script | what it does |
|---|---|
| `sweep_seeds.py`, `bot_quality.py`, `bot_quality_ps2.json`, `loop_spells.py` | the round sweep over seeds, and bot movement against the PS2 |
| `human_profile.py` | a player's profile from a match log, and the fit of the sweep's proxy to it |
| `calibrate_match_length.py` | measure match length and regenerate the menu's model |
| `perf_matrix.py` | the detailed bench over every map |

## Reverse engineering

These read your own disc extract and your own Ghidra exports; nothing here ships game data.

| script | what it does |
|---|---|
| `re_check.py`, `re_calc.py`, `re_calc_selftest.sh` | the main RE checks and arithmetic |
| `annotate_hashes.py`, `analysis_index.py` | annotate name hashes in the listings; generate `analysis/INDEX.md` |
| `ExportCensus.java`, `ExportDecomp.java` | Ghidra scripts that write those exports |
| `callgraph.py`, `scan_tables.py`, `bindings_diff.py` | reachability, Lua binding tables and their BF1/BF2 difference |
| `ucfb_tree.py` | print an LVL's chunk tree |
| `tools/` | small helpers; see `tools/README.md` |

## Project workflow

Reusable agent briefs and standing lane rules are in [docs/agents](../docs/agents/README.md).

| script | what it does |
|---|---|
| `loopctl.py` | retain historical audit prerequisites and explicit reviewed resets |
| `gate_policy.py` | shared gate declaration and landing-group policy |
| `cards.py` | check, list, show and create issue-linked technical cards |
| `heavy.sh`, `cleanse_delete.sh`, `target_cleanup.py` | run a heavy job under the machine's job locks; delete regenerable bloat |
| `claude_guard.sh`, `oracle_guard.py` | agent hooks that refuse risky commands and writes to what the gates judge against |
