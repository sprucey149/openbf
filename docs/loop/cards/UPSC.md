---
id: UPSC
title: Texture upscaling as a public script: scripts/upscale_textures.py fetches a GPU upscaler, sorts each baked texture (prop / building / rock, terrain layer, sky, alpha billboard), runs the right path (4x, 2x with a de-dither pass, or skip) and writes a third look tier the bake picks after the PC copies
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The maintainer (INBOX 2026-10-21 18:40 item 4): "use AI to upscale all the textures to HD, as a py script for openbf so anyone
can". A feasibility study exists (the path is in that INBOX note): an ncnn Vulkan upscaler (BSD-3, no Python ML stack)
does props / buildings / rock well, terrain layers (PS2 dither) and palettised skies badly until a de-dither / deband
pre-pass; alpha kept. `bake::tier` already orders the look tiers (HD pack > PC where larger > ours > PS2).

## Do
- `scripts/upscale_textures.py`: downloads the binary + models, classifies, runs, writes `<name>.tier3.ktx2` beside the
  base; a third tier picked after PC (`OPENBF_TEXTURE_TIER`). Targets: BF1, and every BF2 texture the PC did not enlarge
  (and after XBTEX, every texture no donor enlarged). Each user runs it on their own bake; images never enter git.
- A mip / size cap for the mid-class laptop.

## Acceptance
- A contact sheet per category (before / after) for the maintainer; the tier stays OFF by default until he says yes (STATUS).
- VRAM and level load time measured with the tier on and off on two maps (PERF budget).
- A test: the tier picker takes tier3 only when larger and present, and keeps the PS2 alpha mask.
