---
id: TEXC
title: Block-compressed textures (BC7 colour/alpha, BC5 normals) for the classic-plus tier, and the tier-2 policy: which HD textures ship in the maintainer's build, their mips, the normal-map path
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
The maintainer (INBOX 2026-09-25): "HD textures ship in my build"; the loop decides the classic-plus technical parts; the maintainer reviews
the HD look after M1. Today tier 2 is the PC stock art where larger (`bake/src/tier.rs`), written as uncompressed RGBA8
`<name>.tier2.ktx2` beside the PS2 base: 363 files, 398 MB of the 823 MB of baked map textures (planner, `du`,
2026-09-26). PLAN §Optimisation B5: the Harrisonfog/upscale set would be ~250 MB per map uncompressed — block
compression is what makes shipping HD in one file (PACK) sane.

## Evidence
PLAN §Classic-plus assets (runtime toggle, both tiers resident, sim reads only the PS2 bake) and §Optimisation B5 /
sequence item 1; `bake/src/tier.rs` header (the chain Harrisonfog > PC-where-larger > upscale > PS2; tier 2 takes the
PS2 alpha); `game/src/assets.rs` `TextureTier` (`OPENBF_TEXTURE_TIER`).

## Acceptance (defaults taken by the planner — reversible, record each in the report)
1. **Tier 2 only is compressed:** BC7 (sRGB) for colour/alpha, BC5 for any normal map; mips generated from the
   full-size art, box-filtered in linear light. The PS2 base tier stays uncompressed RGBA8 (palettized art is tiny and
   BC7 would move every golden). A pure-Rust encoder (no C toolchain for PACK); name the crate in CRATES.md.
2. **Runtime:** wgpu `TEXTURE_COMPRESSION_BC` requested; on an adapter without it the loader decodes to RGBA8 (or
   falls back to the PS2 tier) — never a crash. `OPENBF_TEXTURE_TIER=1` renders byte-identical goldens (the PS2 path).
3. **Silhouette invariant held:** the cutout alpha of a compressed tier-2 texture is re-thresholded so the tier flip
   still draws the same silhouette (tier.rs's rule); a test over every cutout tier-2 texture.
4. **Numbers in the report:** tier-2 bytes before/after, bake time, load time per map, and a tier-2 PSNR vs the
   uncompressed tier 2.
5. **Policy note** in `bake/src/tier.rs`: which sources tier 2 takes today (PC-where-larger), what waits for the
   upscale / Harrisonfog layers, and that the normal-map path (tangents: PLAN) lands with the first normal maps.
Gates: sim / sweep / arena untouched; goldens unchanged (they render the PS2 tier); bake DIFF only in `*.tier2.ktx2`.

## Notes

## Planner note (2026-09-30, M-ALPHA)
`defer: post-alpha` — not needed for the alpha's six showcase maps or its checklist. Value 2 (kept).
