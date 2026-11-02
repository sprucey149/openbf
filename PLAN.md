# openbf - PLAN

One game from two: a single Rust codebase for Windows, Linux and macOS that plays like the PS2 Battlefronts (not the PC
ports), with all maps of both games, BF2's shell and HUD, BF1's loading screens, and a "classic-plus" wgpu look over the
same content. Players bring their own discs; we ship no Pandemic assets or code.

**Current scope: Instant Action**, including flying vehicles, space battles and space maps. No new features beyond agreed
tasks. Everything after M3 below is later work and is not being built now.

## Architecture (locked)
1. **Rust runtime + wgpu renderer**; cross-platform via Vulkan/Metal/DX12.
2. **Offline asset bake** from the player's discs; the runtime never parses LVL.
3. **Data-driven missions, no Lua VM.** The Lua mod ecosystem is not a goal (2026-08-30, `analysis/community-projects.md`):
   a mod worth having gets ported as a native feature.
4. **Decomp = behavioral reference**, never source to port; PCSX2 side by side is the oracle.
5. **Same content, classic-plus look.** Same models, maps, silhouettes and palette; modern lighting on by default, a strict
   render mode as a runtime toggle that never changes gameplay.
6. **Modes are composable rulesets over rosters.** Mode x map x era is data, not code.
7. **Session cheat registry**: BF2-style pause-menu codes that reset on restart.

## Decisions

The full text of every decision, with its numbers, is `analysis/plan/FEATURE_SET.md`. Read that section before touching an
area; changing a decision is the maintainer's call. Citations such as `PLAN §Heroes`, `§Modes & rules` or `§Classic-plus
assets` mean the section of that name there. The ones most often needed:

- **Same game everywhere.** The PS2 is the gameplay reference. Every build plays the same inside the playable bounds,
  whichever discs or versions went in, so multiplayer stays possible. Other versions change only the size and look of
  textures and assets. Public correction tables of gameplay numbers and names are approved, preferring instructions on
  the player's own files.
- **Inputs:** one source per game, identified by content. Any PC input needs a PS2 or Xbox BF2 disc too. From the DLC only
  Kit Fisto and Asajj Ventress are taken.
- **HUD:** BF2's on every map, with the heat ring around the reticle. BF1 maps throw BF1 grenades, with BF2's grenade HUD.
- **Soldiers and vehicles:** vehicle and unit health are separate. Droidekas sprint only as a ball, on the shared bar. BF1
  passive aim assist on foot, on by default, none on the mouse. Keyboard + mouse is a deliberate deviation from the
  controller-only original.
- **Bots:** openbf's own AI fed by the discs' graphs and hints; the PS2's behaviour is the baseline, and improvements are
  named deviations.

## Milestones (2026-09-25; older phases are in `analysis/PLAN_LOG_2026.md`)
Cards carry a `milestone:`. Done: P0 audits, P1 bake, P2 viewer, P3 soldier, P4 combat, P5 vertical slice, P6 shell, P6.5
infantry AI A-D, P9's N-pad `Sim::step` and libm-backed `core::fmath`.

- **M0 Health** (always): CI green, tests that test what they say, files at most 1,500 lines (`scripts/size_report.sh`), disk
  hygiene.
- **M-ALPHA, the public alpha** (2026-09-29): a full conquest round with drivable vehicles on six showcase maps (BF1 NAB1,
  TAT1, YAV2; BF2 UTA1, MYG1, HOT1) that reads as a real game in a 5-minute video. Cards carry `alpha: yes`.
- **M1 Looks right:** every map reads like the PS2 under the modern-port rules of
  `engine/artifacts/pcsx2/ref/COMPARISON_RULES.md`; every high-confidence finding in `engine/artifacts/pcsx2/ref/FINDINGS.md`
  fixed or ruled expected.
- **M2 Vehicles:** every vehicle class each map ships spawns, and is drivable or flyable by player and AI with its weapons,
  turrets and enter/exit.
- **M3 Plays right:** a full conquest round on every map ends by tickets with no unit stuck, sweep `finished 32/32`, PCSX2
  spot checks.

Later, in order, and not current scope:
- **M4** Heroes, Space Assault's remaining rules, awards, class meta-unlock (what Instant Action needs of these is in scope).
- **M5** Campaign, Galactic Conquest, splitscreen (card SSCR: one sim, N cameras, no netcode), polish. The shell ships a
  greyed-out Multiplayer entry behind an `enabled` flag.
- **M6 PACK** (card PACK): one self-contained game file and one script that bakes and compiles it from the player's discs. It
  leaves room for later netcode.
- **M7 Multiplayer:** LAN only, cap 16, the last milestone, held until the maintainer says so. Every LAN card carries
  `needs: maintainer - multiplayer hold` and the `net` crate stays a stub. `analysis/plan/FEATURE_SET.md` says client-server with
  bots on the server, and earlier milestone text said lockstep; that choice is open. The determinism spine stays healthy
  either way.
- Post-1.0: new modes, space and CTF reworks, campaign extension, Galactic Conquest tuning.

## How the project runs
- **Scope and decisions.** The source is public and contributions are welcome. Each player builds from their own discs
  (`scripts/pack.py`); game assets, goldens and packed binaries are not public deliverables. The maintainer owns legal
  questions, feel checks and changes to the feature set. Implementation and acceptance are separate: an implemented feature
  can still owe original-game comparisons.
- **Authority.** Follow `AGENTS.md`, the session's instructions and any active stop controls.
- **One plan.** This file holds goals and decisions. The backlog is the issues and the cards (`engine/artifacts/loop/cards/`,
  `BACKLOG.md` generated by `scripts/loopctl.py`). RE lives in `analysis/`. History is git.
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
- `--headless` (no window; wgpu renders offscreen to a texture)
- `--replay <inputlog>` and `--record <inputlog>` (deterministic input playback/capture)
- `--capture-frame <tick> <out.png>` (offscreen render one frame to PNG)
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

## Where the rest went
| Topic | File |
|---|---|
| Decisions in full | `analysis/plan/FEATURE_SET.md` |
| Optimisation backlog, reference corpora, tech stack (read before any dependency bump), testing ladder, fidelity method, community research, kickoff decisions | `analysis/plan/OPTIMISATION_AND_EXECUTION.md` |
| Deferred design work, phase C and D designs, P7 vehicle entity shape, alpha mask classifier (dated designs; `engine/CRATES.md` and cards say what exists) | `analysis/plan/SPECS_AND_DEFERRED.md` |
| Module map | `engine/CRATES.md` |
| Black box, profiling, bot references | `analysis/dev-tools.md` |
| RE | `analysis/INDEX.md`, `analysis/RE_LOG.md` |
| Old log, phases, per-phase acceptance | `analysis/PLAN_LOG_2026.md` |
