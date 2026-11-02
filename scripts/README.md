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
| `gameplay_table.py`, `gameplay_foliage.py`, `gameplay_corrections.py`, `gameplay_programs.py` | generate the PS2 reference numbers and the corrections applied to a player's own files |
| `version_census.py`, `version_geometry.py`, `version_collision.py`, `version_heights.py`, `version_sky.py`, `version_terrain_play.py` | measure how editions differ: names and numbers, never assets |
| `version_report.py`, `version_parity_check.py` | the measurement receipt, and seeded rounds across source combinations |
| `data/` | the measurement receipt and the UI dependency list |

## Checks

| script | what it does |
|---|---|
| `gates.py`, `gates_selftest.py`, `gates_history.py` | run the gates and compare them with the stored values; test the shortcuts; replay a golden skip rule over history |
| `sim_gates.sh`, `gpu_tests.sh`, `goldens.sh` | sim referees, GPU referees, golden pictures |
| `fdiff.py` | first divergence between two sim traces |
| `parity.py` | score the port against stored PS2 frames |
| `sound_bake_check.py`, `audition.sh`, `flythrough.sh` | check baked sounds; listen to a bank; one orbit video per map |
| `size_report.sh`, `size_waivers.txt`, `check_provisional.py` | file sizes and the crate map; provisional values must name their measurement |
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

| script | what it does |
|---|---|
| `loopctl.py`, `land.py` | card bookkeeping and landing |
| `heavy.sh`, `cleanse_delete.sh` | run a heavy job under the machine's job locks; delete regenerable bloat |
| `start_loop.sh`, `autoloop.sh`, `loop_cpu.py`, `loopbin/`, `AUTOLOOP.md` | the older unattended loop |
| `claude_guard.sh`, `oracle_guard.py` | agent hooks that refuse risky commands and writes to what the gates judge against |
