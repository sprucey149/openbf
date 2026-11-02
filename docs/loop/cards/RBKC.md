---
id: RBKC
title: VTUR2's build_zone never reached the shared common bake: common_bf1/bf2 common.ron (baked 09-30 11:07, before VTUR2) lack build_zone on com_prop_barrier_02 / com_item_healthrecharge_unbuilt, so every full-bake gate DIFFs 2 files and BF2's unbuilt health droids draw their built row
milestone: M2
value: 4
phase: build
status: done
deps:
files:
declares: rebake=common; goldens=<whatever the unbuilt health droids move — view at land>
needs:
branch:
resume:
created: 2026-09-30
alpha: yes
---
Found by AUDRB's full scratch bake of main 7dfe311 (engine/artifacts/lead_tmp/audrb/gates/cmp_bake.txt: SAME=22557 DIFF=2, both common_*/common.ron, each +2 build_zone: Some("com_prop_buildzone") lines). VTUR2 declared rebake=maps,classes,sounds but not common. game::assets::resolve_class sends a BF2 class with build_zone to its zone's row, so the rebake changes what BF2 maps draw/collide where com_item_healthrecharge_unbuilt is placed (on most BF2 maps) — gate it through land.py, not a hand rebake.

## Closed (planner 2026-09-30 17:30)
Resolved by BHUD's landing: its `rebake=common` (b06686f, 13:34) wrote both `build_zone: Some("com_prop_buildzone")` lines into the shared `common_bf1/common.ron` and `common_bf2/common.ron` (re-checked: 2 lines each), and main's gpu gate after that rebake read goldens unchanged (`reports/20260930_1340_resume_BHUD.md` Decisions). Nothing left to land. Reverse: `loopctl.py set RBKC status=todo`.
