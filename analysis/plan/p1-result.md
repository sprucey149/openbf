# P1: personal names out of source comments (2026-11-27)

**What changed.** The `SCRUB` table and `CAP` rule from `scripts/export_public.py` were applied to the
`//`-comment part of each line in 73 `.rs` files under `engine/crates` (158 lines). The name becomes
"the maintainer" / "The maintainer", and the earlier wgpu project's name becomes "an earlier wgpu
project". Code, strings, paths and fixture names were not touched. Every hit was already inside a comment;
none were in strings or identifiers. The two regex-only hits were checked by eye:
`bake/src/sounds.rs:899` ("a tool of the maintainer's") and `core/src/sim/state/events.rs:196`
("hitmark, the maintainer 2026-10-28"). Both read correctly.

**Skipped (performance-chain files), 8 mentions left:** `game/src/blackbox.rs` (2),
`game/src/soldier_vis/tests.rs` (3), `game/src/viewer/scene.rs` (2) and `render/src/lib.rs` (1). The
last two were added to the skip list because the PERF plan and branches touch them
(`claude/perf-0` and `claude/perf-bench` edit `render/src/lib.rs`; PERF split work names `viewer/scene.rs`).

**Non-`.rs` files (review follow-up).** Two comment hits outside `.rs` were fixed:
`input/pads.txt:7` ("verified by the maintainer", a `#` line the mapping loader skips) and
`render/src/shader.wgsl:26` ("adapted from an earlier wgpu project's mesh shader"). Also left for after
PERF: `render/src/pipelines.rs:32` and `:915` name the other project bare ("Hangar `pipeline_m.rs`"), which
the export table does not scrub; `pipelines.rs` is in the PERF plan.

**Checks.**
- `grep -rnwi -E 'maintainer|user|maintainer.s|friends|an earlier wgpu project' engine/crates --include=*.rs`: 166 hits before,
  8 after, all in the four skipped files. Re-run without `--include=*.rs` (all files): same 8.
- `cargo build --release -p game`: ok.
- `cargo clippy --workspace --all-targets -- -D warnings`: fails on 17 existing code lints from the current
  toolchain (clippy 0.1.100: `is_multiple_of`, `unwrap_or`, `?` rewrites, disallowed `f32::max`, ...). All
  of them are on code lines this change does not touch, so this change adds no new warnings.
  `cargo fmt --check` also reports existing diffs; the only touched file among them,
  `core/src/ai/stall.rs:449`, has its diff on a code line.
- `scripts/one_player_gate.py`: every row matches (lan_soak ok).
- No tests run: comment-only change.

**Left.** Once the PERF chain has landed: scrub the 8 mentions in the four skipped files and the two
bare "Hangar" references in `render/src/pipelines.rs` (32, 915).
