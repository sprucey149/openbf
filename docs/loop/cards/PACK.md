---
id: PACK
title: FINAL DELIVERABLE (only on the maintainer's go-ahead): one self-contained game binary (no DLLs / helper files) + one Python script: 2 ISOs -> bake -> compile -> the single binary
milestone: M6
value: 1
phase: build
status: done
deps:
files:
declares:
needs: maintainer — his go-ahead when the core game is basically done; do NOT start before (the maintainer, 2026-09-25)
effort:
branch:
resume: landed 530dc45 2026-10-18
created: 2026-09-25
defer: post-alpha
---

## The maintainer's spec (2026-09-25)
Personal use only (the maintainer + local multiplayer / splitscreen with friends; never on the internet), so legal/sharing is not a concern — but assets still never go into git. At the END of the project, not during dev work:
1. **One file is the whole game**: the release binary with every baked asset inside it and no DLLs or helper files next to it (static linking where the platform allows; the system GPU driver / Vulkan loader is the one thing a Linux/Windows game cannot carry).
2. **One Python script** that takes the two ISOs, runs the whole extraction + bake the loop uses today, compiles the Rust source and produces that single binary. With assets already baked it can skip straight to compiling.
Dev builds stay as they are (small binary + `engine/assets_baked/`), because compiling 1.6 GB+ of assets into every build is slow.
The earlier design (exe-relative asset paths, then append an archive + footer to a copy of the release exe) is the likely route.

## Evidence
The maintainer 2026-09-01; spec in `analysis/PLAN_LOG_2026.md` Part 2 §NEWLY OPUS-SAFE 2026-09-01 #6; analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md §Standing backlog 5.

## Note (planner 2026-10-03 07:15, from AMBM's landing 1bde70c)
The battle's music is not baked: the runtime streams each take's ADPCM out of the game's own SOUND LVLs, found under
the repo's unpacked disc tree (the environment variable `OPENBF_EXTRACTED` moves it; no file: no music, one line on
the terminal). That is a dev layout. The packaged game needs the takes inside its own data: either the bake copies
the stream bodies it names (the `mus_` records already carry each take's place and size), or the single binary
embeds them. `audio/src/stream.rs` and `game/src/sounds.rs` (`music`) are the two places that know the path.
