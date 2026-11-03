---
id: STLC
title: A render with no battle draws no building whose class is in the COMMON bake: BF1's command post buildings and BF2 Death Star's post base are missing from pose_render (the parity score's port frames) and the goldens' ground views, while the PS2 frame beside them shows the post
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Found by card CPHOL (2026-10-13): the hologram now stands over nothing in the port's still frames. A map's own class table (`Scene::resolve_class`) has no `com_bldg_controlzone` / `com_bldg_major_controlzone` entry on these maps (they are in `common_<game>/common.ron`); a battle loads the common bake (`main/battle.rs` `load_common`), the still paths do not. [H] that this is the whole cause: trace one post instance through `upload_scene` first.

## Evidence
- `engine/artifacts/lead_tmp/cphol/pair_bf1_end1_cp1.png` (PS2 left: the post under the green column; port right: the column alone), `w_b1kas.png`, `dea_cp3_pair1.png`, `kam_capture_sheet.png`.
