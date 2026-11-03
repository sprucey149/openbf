# Agent brief template

Copy one lane section into a brief and fill the placeholders. Give every worker a
bounded task, owned files, a source revision and concrete completion evidence.

## Shared rules

Read AGENTS.md, PLAN.md and the relevant technical evidence before changing files.
openbf's gameplay rules are canonical; the original games are the evidence and deliberate
differences are named. Every source plays the same game, with BF2's HUD on
every map. Honor ownership, keep game data out of Git and stage files by name.
Never work around a denied command. Keep automated runs headless. Run GPU tests
through `scripts/gpu_tests.sh` with `--test-threads=1`.

## Build lane

- Task and issue link — `<player-visible scope and issue>`.
- Base revision and owned files — `<revision, paths, excluded concurrent work>`.
- Evidence to read — `<technical card and reference measurements>`.
- Completion evidence — `<focused commands, expected counts and retained results>`.
- Result — commit the scoped implementation and record source SHA, checks, remaining
  acceptance and next steps in the assigned technical record.

## Review lane

- Target — `<branch, base and exact source SHAs>`.
- Reproduce the claimed behavior and attack failure, retry and concurrent paths.
- Review deletions and current callers. Check the live main merge and stale docs.
- Result — report numbered findings with file and line, minimal fixes and a verdict.
  Separate reproduced checks from static review and unchecked acceptance.

## Land lane

- Target — `<reviewed branch and review evidence>`.
- Focused checks — `<commands to run on the branch and combined main>`.
- Public issue update — `<player-facing result, checks and remaining scope>`.
- Result — record merged source and publication SHAs and update technical evidence.
  Preserve contributor history and commit README edits separately.
