# Subsystem Navigation Guide

This document maps player-visible subsystems to their entry points, implementation files, test commands and evidence tools. Current status lives in the GitHub issues, not here.

## Authoritative Documentation Homes

To prevent duplicate explanations or stale contradictions, each topic has one authoritative source of truth:

| Topic | Authoritative Home | Notes |
|---|---|---|
| **Product Decisions & Scope** | `PLAN.md` | Feature decisions, platform support, non-goals. |
| **Crate & Module Map** | `engine/CRATES.md` | Module ownership, line counts, standing invariants. Check via `scripts/size_report.sh --check-map`. |
| **Reverse Engineering Notes** | `analysis/` (indexed by `analysis/INDEX.md`) | PS2 ELF functions, field offsets, game mechanics. Check via `scripts/analysis_index.py`. |
| **Black Box & Diagnostics** | `analysis/dev-tools.md` | F9 black box snapshot format, debug flags, offline parsers. |
| **Developer Scripts & Tooling** | `scripts/README.md` | CLI scripts, testing tools, bake orchestration. |
| **Task Ownership & Status** | [GitHub issues](https://github.com/sprucey149/openbf/issues) | Work list, ownership, current status and remaining acceptance. Historical evidence stays in `docs/loop/cards/`. |

---

## Subsystem Navigation Map

### 1. Gameplay & Simulation AI
- **Player-Visible Scope**: Unit movement, hero sprint/jump attacks, weapon firing, vehicles, AI bot navigation, tickets, capture posts.
- **Entry Points**: `engine/crates/game/src/main.rs`, `engine/crates/game/src/main/`
- **Core Implementation**:
  - Simulation loop: `engine/crates/core/src/sim/step.rs`
  - Bot navigation: `engine/crates/core/src/ai/route.rs`, `engine/crates/core/src/ai/behave/brain.rs`
  - Soldier motor & weapons: `engine/crates/core/src/soldier.rs`, `engine/crates/core/src/weapon.rs`
  - Tow-cable mechanics: `engine/crates/core/src/vehicle/tow.rs`
- **Verification Commands**:
  - Unit tests: `cargo test --release -p core --lib`
  - Deterministic hash pin: `cargo test --release -p core --lib original_hash_pins_without_tracing -- --nocapture`
  - Fast simulation hash: `cargo run --release -p game -- --sim-hash 100` (must match the pinned hash in the core determinism test)
- **Evidence & Debugging**:
  - F9 blackbox snapshot: `cargo run --release -p game --example blackbox_read -- path/to/snapshot.json`
  - Long simulation sweeps: `python3 scripts/sweep_seeds.py --seeds 1..3 --maps tat1,hot1`

### 2. Missions & Input Profiles
- **Player-Visible Scope**: Game profile selection (`ps2-only`, `pc-only`, `xbox-only`, `mixed`), mission playlist generation, era rosters.
- **Entry Points**: `scripts/input_scan.py`, `scripts/gen_missions.py`
- **Core Implementation**:
  - Manifest schema: `engine/crates/manifest/src/lib.rs`
  - Mission definition loading: `engine/crates/game/src/mission.rs`, `engine/crates/game/src/roster.rs`
- **Verification Commands**:
  - Manifest roundtrip: `cargo test --release -p manifest --lib`
  - Input scan check: `python3 scripts/input_scan.py`
- **Evidence & Receipts**: `openbf.inputs.json`, `openbf.sources.json` written alongside builds.

### 3. Assets & Offline Baking
- **Player-Visible Scope**: Asset extraction from retail discs/installs, geometry, textures, audio decoding.
- **Entry Points**: `scripts/pack.py`, `scripts/rebake.sh`, `scripts/pack_profiles.sh`
- **Core Implementation**:
  - Disc format parsing: `engine/crates/mungefs/src/lib.rs`
  - Asset conversion: `engine/crates/bake/src/map.rs`, `engine/crates/bake/src/sounds.rs`
  - PS2 gameplay correction tables: `engine/crates/bake/src/gameplay.rs`
- **Verification Commands**:
  - Unit tests: `cargo test --release -p mungefs --lib`, `cargo test --release -p bake --lib`
  - Deterministic bake diff: `scripts/cmp_bake.sh`
- **Evidence & Receipts**: the local `engine/assets_baked/` tree (never committed); `scripts/cmp_bake.sh` compares two bakes.

### 4. Rendering & User Interface
- **Player-Visible Scope**: Headless and windowed graphics rendering, 3D meshes, particle effects, HUD, fonts, minimap.
- **Entry Points**: `engine/crates/render/src/lib.rs`, `engine/crates/ui/src/lib.rs`
- **Core Implementation**:
  - WGPU frame pipeline: `engine/crates/render/src/frame.rs`
  - Shader WGSL sources: `engine/crates/render/src/*.wgsl`
  - BF2 Unified HUD: `engine/crates/ui/src/hud.rs`
- **Verification Commands**:
  - Headless GPU tests: `scripts/gpu_tests.sh --render` (run serially with `--test-threads=1`)
  - Golden image comparison: `scripts/goldens.sh`
- **Evidence**: original-game captures compared by `scripts/goldens.sh`; goldens stay local.

### 5. Audio & Sound Effects
- **Player-Visible Scope**: 3D sound positioning, voice overs, unit death sounds, music streaming.
- **Entry Points**: `engine/crates/audio/src/lib.rs`, `engine/crates/game/src/sounds.rs`
- **Core Implementation**:
  - Sound property parsing: `engine/crates/mungefs/src/sound_props.rs`
  - Playback and streaming: `engine/crates/audio/src/lib.rs`, `engine/crates/audio/src/stream.rs`
- **Verification Commands**:
  - Integration tests: `cargo test --release -p game --test suite -- hero_audio`
- **Evidence**: F9 sound event records, retail audio recordings.

### 6. Packaging & Distribution
- **Player-Visible Scope**: Standalone executable package (`openbf` / `openbf.pak`).
- **Entry Points**: `scripts/pack.py`, `scripts/pack_profiles.sh`
- **Core Implementation**:
  - Archive format: `engine/crates/pak/src/lib.rs`
  - Package writer CLI: `engine/crates/game/src/main.rs` (`--write-pack`)
- **Verification Commands**:
  - Package limit tests: `python3 -m unittest scripts/tests/test_pack_limits.py`
  - Multi-profile pack: `scripts/pack_profiles.sh`
- **Evidence**: `openbf.inputs.json` and `openbf.sources.json` written beside each package.
