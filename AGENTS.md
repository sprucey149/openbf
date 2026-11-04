# openbf agent instructions

openbf is a Rust port of Star Wars Battlefront (2004) and Battlefront II (2005).
openbf has one gameplay rulebook that every build follows. The original games are the
evidence, and deliberate differences are named. Players build it from their own copies.

Read `PLAN.md` for scope and decisions, `engine/CRATES.md` for the module map,
`analysis/INDEX.md` before reverse engineering, `analysis/dev-tools.md` for the
black box and profiling, and `scripts/README.md` for tools.

## Working rules

- Instant Action is the scope, including flying vehicles, space battles and space maps.
- Every supported source plays the same game inside playable bounds. Other editions
  change asset size and appearance. Public gameplay correction tables are approved.
- Use BF2's HUD on every map, with the heat ring around the reticle. BF1 maps use BF1
  grenades with BF2's grenade HUD. Unit and vehicle health are separate. Droidekas
  sprint as a ball on the shared bar; their ball jump spends it too. Other decisions are in `analysis/plan/FEATURE_SET.md`.
- Keep game data out of Git and PRs, including meshes, art, audio, disc images,
  extracted scripts, baked assets, packed builds and localized text.
- Explain reverse-engineered behavior in your own words, citing function and line.
  Keep decompiler text out of code, comments and documentation.
- Unlicensed reconstructions are factual references only. Never copy code or close
  paraphrases; public notes may use original words with project names and PC addresses.
  Establish PC leads in the PS2 files and original game before changing Rust.
- Keep `core` deterministic. Use `core::fmath`, seeded simulation randomness and
  fixed ticks. Every new PROVISIONAL, OURS or INTERIM value names its replacement
  measurement with `replace:`, `measure:` or `trace:`.
- Agree scope and ownership before implementing. Honor contributor claims. Keep one
  feature per branch and PR. Stage files by name. Never force-push.
- Run focused checks for touched behavior. Record the commands, source revision,
  counts, evidence and unchecked acceptance. Original-game comparisons require
  original-game evidence.
- Run GPU tests with `--test-threads=1` through `scripts/gpu_tests.sh`. Keep automated
  runs headless. Never work around a denied command.

## Coordination

[GitHub issues](https://github.com/sprucey149/openbf/issues) hold the public to-do
list and ownership. Describe player-visible scope and completion evidence in each
issue. Reserve work before starting, update on landing and close when its own scope
is finished. Preserve contributor history during integration.

See `docs/agents/brief.md` for build, review and land briefs. Historical evidence
remains in `docs/loop/cards/` in the public repository.

## Black box and performance

When the maintainer reports something seen in play, read the F9 black box snapshot
first. `analysis/dev-tools.md` explains its bots, sound and damage records.

When reporting overnight work, use the Git log and current status, and include the
latest performance measurement with its date. State when the per-map 60 fps bot
population or simulation timing remains unmeasured.
