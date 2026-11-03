---
id: ISPD
title: Speed on a weak laptop: the game runs well on an Intel Iris Xe class machine (Surface Pro 8, i7-1185G7, 2880x1920) - measure where the time goes on this machine's iGPU, then fix the biggest costs in order
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
(the maintainer, 2026-10-19) On his Surface Pro 8 (i7-1185G7, Intel Iris Xe integrated graphics, 2880x1920 panel) some maps ran poorly: BF2 Geonosis well under 30 fps. The game must run well on that class of machine. This card is the render side and the whole-frame picture; the AI-CPU side is AITHK, the discs' LOD switch is LOD (first, it is ready).
Facts from the chat session that suggest where the time goes (none measured): the game renders at the window's native resolution; far culling is off by default (`OPENBF_FAR_CULL`); every model draws LOD0 at any distance (LOD); the specular second pass (`OPENBF_SPEC`), BF2's terrain light pass (`OPENBF_TERRAIN_LIGHTS`) and the DomeInfo frame filter (`OPENBF_NO_FILTER`) are each an extra pass over the screen and are on by default; opaque draws are sorted by material, not by depth (overdraw). The bot-count benchmark half of PERF is deferred to the end of the project (the maintainer): this card measures the renderer and the sim at TODAY's bot counts, not the largest count that holds 60 fps.

## Evidence
This machine's Intel iGPU (renderD128) is the stand-in for Iris Xe (`OPENBF_ADAPTER=igpu`); `game --bench` and the `--bench-*` flags exist (`game/src/main.rs`); PLAN §Optimisation backlog; analysis/re-lod.md and the far-scene cards FARS / FARC. Drawing opaque objects nearest-first (or a depth prepass) cuts overdraw on integrated GPUs - a hypothesis to test here, not a result.

## Acceptance
1. A measurement table in `engine/artifacts/ispd/README.md`: per showcase map (GEO1, KAM1, TAT2, a BF1 map, the heaviest map by draw count) at 1920x1080 and 2880x1920 on the iGPU: frame ms, ms per pass (shadow volumes, terrain, models, specular, terrain lights, frame filter, HUD), draw calls, triangles, and the sim's ms per tick; which of them is GPU-bound and which CPU-bound.
2. Build slices, each its own card or commit, ordered by measured saving per effort, each with a before/after on the same table: candidates are far culling on by default (with the PS2's fade at the end of NearSceneRange, see FARS / FARC), depth-ordered opaque draws or a depth prepass, an optional render-resolution scale (a settings entry, default 1.0), the extra passes' cost against their look (nothing is turned off without the maintainer's say; a measured table is the deliverable), frustum culling of instances if absent.
3. No golden moves unless declared and viewed; the sim hash is untouched (render-side only).
4. A target stated back to the maintainer: the frame rate each shipped map reaches on the iGPU at 1080p and native after the slices.

## Notes
Do not edit LOD's files while LOD is in progress (render/src/draw.rs, mesh.rs); slices wait for it. The maintainer does not want gameplay changed for speed.
- planner 2026-10-19 16:25: from the maintainer's 10-19 INBOX list, two more candidate slices to price on the same table: **animation LOD** (the animator's ms per frame at today's bot counts first; if it is a real cost, cut a card that finds the PS2's own low-detail animator - `analysis/re-lod.md` says ~4 soldiers a frame get the hi-res body - presentation only, no sim hash), and **nearest-first opaque draws** (a depth bucket in `draw_key`, or a depth prepass). FARC (un-deferred today) is the RE that lets `OPENBF_FAR_CULL` default on. Every constant from the PS2 corpus. While PERF's bot-count benchmark is deferred (the maintainer 10-19), this card's `engine/artifacts/ispd/README.md` table is the performance line STATUS copies.
