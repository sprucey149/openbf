# openbf - PLAN

One game from two. openbf brings Battlefront and Battlefront II together in a Rust game for Windows, Linux and macOS.
It uses one gameplay rulebook, BF2's shell and HUD, BF1's loading screens, and modern wgpu rendering over the original
maps. Players build it from their own supported copies. All original game data comes from those local inputs.

**Current scope: Instant Action**, including flying vehicles, space battles and space maps. No new features beyond agreed
tasks. Everything after M3 below is later work and is not being built now.

## Architecture (locked)
1. **Rust runtime + wgpu renderer**; cross-platform via Vulkan/Metal/DX12.
2. **Offline asset bake** from the player's supported copies. LVL parsing happens exclusively during baking.
3. **Data-driven missions, no Lua VM.** The Lua mod ecosystem is not a goal (2026-08-30, `analysis/community-projects.md`):
   a mod worth having gets ported as a native feature.
4. **Original-game evidence.** Use the original games, executable analysis and PCSX2 comparisons to verify behaviour, and implement it in original Rust code.
5. **Same content, classic-plus look.** Same models, maps, silhouettes and palette; modern lighting on by default, a strict
   render mode as a runtime toggle that never changes gameplay.
6. **Modes are composable rulesets over rosters.** Mode x map x era is data, not code.
7. **Session cheat registry**: BF2-style pause-menu codes that reset on restart.

## Decisions

The [feature set](analysis/plan/FEATURE_SET.md) holds the current gameplay and
presentation decisions, including deliberate differences from the originals.
References such as `PLAN §Heroes` and `PLAN §Modes & rules` point to those sections.
Changing a gameplay decision is the maintainer's call.

## Milestones (2026-09-25; older phases are in `analysis/PLAN_LOG_2026.md`)
GitHub issues track current milestone scope. Done: P0 audits, P1 bake, P2 viewer, P3 soldier, P4 combat, P5 vertical slice, P6 shell, P6.5
infantry AI A-D, P9's N-pad `Sim::step` and libm-backed `core::fmath`.

- **M0 Health** (always): CI green, tests that test what they say, files at most 1,500 lines (`scripts/size_report.sh`), disk
  hygiene.
- **M-ALPHA, the public alpha** (2026-09-29): a full conquest round with drivable vehicles on six showcase maps (BF1 NAB1,
  TAT1, YAV2; BF2 UTA1, MYG1, HOT1) that reads as a real game in a 5-minute video. The alpha issue scope records these targets.
- **M1 Looks right.** Every map retains the look of its original game under the modern-port presentation rules.
  Every high-confidence comparison finding needs a fix or a recorded explanation.
- **M2 Vehicles:** every vehicle class each map ships spawns, and is drivable or flyable by player and AI with its weapons,
  turrets and enter/exit.
- **M3 Plays right.** Acceptance requires full Conquest rounds on all 34 ground maps to end by tickets with no units stuck,
  a `finished 34/34` sweep footer, and original-game spot checks.

Later, in order, and not current scope:
- **M4** Heroes, Space Assault's remaining rules, awards, class meta-unlock (what Instant Action needs of these is in scope).
- **M5** Campaign, Galactic Conquest, splitscreen (card SSCR: one sim, N cameras, no netcode), polish. The shell ships a
  greyed-out Multiplayer entry behind an `enabled` flag.
- **M6 PACK** (card PACK): one self-contained game file and one script that bakes and compiles it from the player's discs. It
  leaves room for later netcode.
- **M7 Multiplayer:** LAN only, cap 16, the last milestone, held until the maintainer says so. Every LAN issue remains on the maintainer’s multiplayer hold and the `net` crate stays a stub. `analysis/plan/FEATURE_SET.md` says client-server with
  bots on the server, and earlier milestone text said lockstep; that choice is open. The determinism spine stays healthy
  either way.
- Post-1.0: new modes, space and CTF reworks, campaign extension, Galactic Conquest tuning.

## How the project runs
- **Scope and decisions.** The source is public and contributions are welcome. Each player builds from their own discs
  (`scripts/pack.py`); game assets, goldens and packed binaries are not public deliverables. The maintainer owns legal
  questions, feel checks and changes to the feature set. Implementation and acceptance are separate: an implemented feature
  can still owe original-game comparisons.
- **Authority.** Follow `AGENTS.md`, the session's instructions and any active stop controls.
- **Work and evidence.** [GitHub issues](https://github.com/sprucey149/openbf/issues)
  hold the public work list and contributor assignments. Each issue has one technical
  card for implementation, verification and remaining acceptance. The generated backlog
  indexes the cards. The [analysis index](analysis/INDEX.md) leads to original-game research.
- **Parallel work.** Agree scope and ownership first; honor other contributors' claims; verify combined behaviour before
  integration.
- **Gates** (`scripts/gates.py`, `engine/artifacts/loop/gates.toml`): CI, GPU tests and goldens, sim hashes, the round sweep,
  the arenas, byte-identical bakes. Goldens that move are declared, quantified and viewed before they are committed.
- **RE discipline** (`scripts/re_check.py`): [V]/[D]/[H] tags, results marked `[O] UNAUDITED` until an auditor re-derives
  them, and PROVISIONAL on every unproven constant.


## Determinism & capture/replay spine
The sim is **fixed-step** (rate from audit A5), **seeded RNG**, **no wall-clock in sim
logic** (two diagnostic timers, `ai_wall_ms` in `core/src/sim/step.rs` and `TableStats.secs` in
`core/src/ai/tactic.rs`, are never hashed or read by the sim; `core::fmath`'s `no_wall_clock_in_core`
referee allows only those). The `game` binary must support from the start:
- `--map <baked-dir> --headless` (render offscreen)
- `--replay <inputlog>` (deterministic input playback)
- `--map <baked-dir> --headless --capture-frame <out.png>` (render one frame to PNG)
- `--sim-hash <ticks>` (run N ticks, print a hash of sim state)
This single architecture is what makes autonomous testing possible (golden frames,
replay diffs, state hashing) AND is the foundation for lockstep/rollback netcode. Treat
it as non-negotiable.

## Guardrails (legal and safety, never violate)
- The engine is original code. This is interoperability work with the player's own discs.
- **Never commit or upload** copyrighted assets (models, textures, audio, movies), extracted ELFs, decompiled C or baked
  assets. All are gitignored. Do not distribute a playable build containing LucasArts/Pandemic IP.
- Keep `iso/`, `extracted/` and ELFs read-only.
- Public publication follows `AGENTS.md`. The repo may ship a table of gameplay numbers and names, and nothing more of the games.

## Reference documents
| Topic | File |
|---|---|
| Decisions in full | `analysis/plan/FEATURE_SET.md` |
| Optimisation backlog, reference corpora, tech stack (read before any dependency bump), testing ladder, fidelity method, community research, kickoff decisions | `analysis/plan/OPTIMISATION_AND_EXECUTION.md` |
| Deferred design work, phase C and D designs, P7 vehicle entity shape, alpha mask classifier (dated designs; `engine/CRATES.md` and cards say what exists) | `analysis/plan/SPECS_AND_DEFERRED.md` |
| Module map | `engine/CRATES.md` |
| Black box, profiling, bot references | `analysis/dev-tools.md` |
| RE | `analysis/INDEX.md`, `analysis/RE_LOG.md` |
| Old log, phases, per-phase acceptance | `analysis/PLAN_LOG_2026.md` |
