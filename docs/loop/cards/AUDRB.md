---
id: AUDRB
title: The shared bake's audio groups predate the 2026-09-20 ffmpeg upgrade: a full scratch bake DIFFs ~5.8k .ogg files (6 bytes each: the Lavc62.28.102 -> .103 vendor string), so every mungefs/bake landing's full-bake gate reads MOVED
status: archived
---

Historical technical record. Recorded state before archival was `done`.

Found by KOC's gates (engine/artifacts/lead_tmp/koc/gates/cmp_bake.txt: 5345 audio/global_bf2, 488 audio/global_bf1, plus the maps' sounds/*.ogg; strings shows Lavc62.28.102 in assets_baked, .103 in the scratch). Not a code change: rebake the audio groups (and let the next maps rebake refresh the per-map oggs), then check that a full scratch bake gives DIFF=0. Behaviour-neutral: the audio is byte-identical apart from the encoder tag.

## Closed 2026-09-30 (lane 2): already done by VTUR — measured, no rebake needed
- Shared bake: all 5865 `.ogg` carry `Lavc62.28.103`, 0 carry `.102` (grep over engine/assets_baked). VTUR's landing
  (reports/20260929_0340_card_VTUR.md) declared `sounds,global-sound` for exactly this; audio dirs dated 2026-09-29 02:42.
- Fresh full scratch bake of main 7dfe311 (`gates.py --groups bake`, lead_tmp/audrb/gates/): SAME=22557 DIFF=2
  ONLY_IN_A=0 ONLY_IN_B=0 over 22559 files (5865 .ogg, all SAME). BFLT's branch full-bake gate today: 0 .ogg DIFFs too.
- The 2 DIFFs are `common_bf1/bf2/common.ron` (VTUR2's build_zone, common never rebaked) → new card RBKC.
