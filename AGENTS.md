# openbf - notes for every agent session

openbf is a Rust port of Star Wars Battlefront (2004) and Battlefront II (2005). The PS2 games are the gameplay
reference. Players build it from their own discs.

Read next: `PLAN.md` (goals and decisions), `engine/CRATES.md` (module map), `analysis/INDEX.md` (grep it before any
reverse engineering), `analysis/dev-tools.md` (black box, profiling), `scripts/README.md`.


## Rules

- **Instant Action is the scope**, including flying vehicles, space battles and space maps. No new modes or features
  beyond agreed tasks.
- **The PS2 is the gameplay reference, and every build plays the same game inside the playable bounds**, whichever
  supported source it was built from, so multiplayer stays possible. Other versions change only the size and look of
  textures and assets. Public correction tables of gameplay numbers and names are approved, preferring instructions
  applied to the player's own files. The repo holds no meshes, art, audio, script bytecode or localized text.
- **Standing decisions:** BF2's HUD on every map, with the heat ring around the reticle. BF1 maps throw BF1 grenades, with
  BF2's grenade HUD. Vehicle and unit health are separate. Droidekas sprint only as a ball, on the shared bar. The rest
  are in `analysis/plan/FEATURE_SET.md`; changing one is the maintainer's call.
- **Game data never enters Git or a PR:** no textures, models, audio, disc images, extracted scripts, baked assets or
  packed builds. Decompiled text never goes into code or docs.
- **Keep the simulation deterministic:** no wall clock, OS randomness or host float calls in `core`; use `core::fmath`.
- A new value labelled PROVISIONAL, OURS or INTERIM says what measurement replaces it (`replace:`, `measure:` or `trace:`
  in its comment); `scripts/check_provisional.py` checks added lines and `engine/ci.sh` runs it.
- Say what you checked and what you did not. A partial check, a fixture or a skipped original-game comparison does not
  complete a task that requires it.
- Stage files by name. Never force-push.

## Coordination

Public issues describe what a player sees, the intended result, the scope and the evidence that finishes it, in readable
titles, with card IDs only as references. Group implementation slices under their feature, and do not open an issue
for each internal card or routine check. Say when code already exists. Agree on scope and ownership with the maintainer
before implementing, and honor tasks another contributor has claimed. One feature per branch and PR, with tests and
before/after evidence; agree on wider refactors first.

Maintainer-run agents may commit and push reviewed, validated work on the maintainer's behalf. One integration coordinator
publishes to upstream main; other workers and contributors use their own branches and PRs.

Cards are in `engine/artifacts/loop/cards/` here and `docs/loop/cards/` in the public repository. They hold specifications
and history; a finished card keeps its result and evidence only.

## When the maintainer reports something he saw

Read the black box snapshot first: `analysis/dev-tools.md` says how, for bots, sounds and damage.

## When asked what happened overnight

Report from the git log and `engine/artifacts/loop/STATUS.md`, and include the performance line from
the top of `engine/artifacts/loop/PERF.md` (largest bot count per map holding 60 fps, sim ms per tick, what changed).
Say so if it is not current. Neither file is published, nor are `INBOX.md` and the JOURNAL.
