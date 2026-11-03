---
id: TSLG
title: A turret stands at the base of a leg instead of on the platform it belongs to (the maintainer's playtest 2026-10-07, BF2 Kamino): find the platform and every turret slot like it, and put each turret where the PS2 puts it (the pad / hardpoint of the stand, not the stand's origin)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The maintainer, 2026-10-07: "At the end of the round I noticed a platform that normally hosts a turret but there was no turret there. A turret was at the base of this leg thing - I think the turret should be on this leg thing as a turret station."

## Evidence
- The snapshot cannot name the platform: the maintainer stood at (-125.6, 70.2, 151.0) on BF2 KAM1 and looked 6 m ahead at (-128.1, 70.2, 151.5); no turret or platform class lies within 25 m of that spot (nearest: `tur_weap_built_gunturret1013` at (-102.0, 69.3, 142.3), `kam_prop_crate`s, and `kam_bldg_tower2` / `pillar2` / `landplatform1` at about (-84, 69, 146), 42 m). So he saw it from there or elsewhere in that round - ASK HIM or find it (acceptance 1).
- KAM1's manifest has 33 `tur_*` instances (y 51.4 - 79.4; six at 51-53 m on the low decks, the rest at 69-79). A turret stands where its instance's translation says; nothing in the port moves it to a stand's hardpoint. `tur_bldg_chaingun_roof` instances (`help`, `ambush`, `...roof10`) are roof turrets.
- Related: TBLD (build pads become turrets), VTURD (a turret's gun turns, damage shows), TURR (bots man turrets).

## Notes
Filed 2026-10-07 by Claude in chat from the maintainer's playtest (BF2 Kamino, F9 at tick 10701, build 6f6160b; snapshot `playtest/prev1.json`). His words are quoted; nothing here is measured unless it says so. A question for the maintainer in STATUS: which post / which side of the map, or press F9 standing next to the platform (the snapshot's `focus.point` is the look point).
- 2026-10-07 11:30 (the maintainer, second F9, via Claude): "it's platforms like these. There should be turrets on all these platforms, and grenades if thrown correctly fall through the floor." Screenshot `playtest/playtest_20261007/5_kam1_platform_no_turret.png`, snapshot `playtest/playtest_20261007/f9_kam1_t3481.json` (BF2 KAM1, tick 3481, build 6f6160b): he stands at (103.1, 52.4, 75.0) on a large low deck (y 52.4), rail ring with lit strips, a floor grid, two raised blocks on the near rail, rain. This is a DIFFERENT place from the first F9 (-125.6, 70.2, 151.0, a high deck at y 70), so it is a kind of platform, not one spot.
- From the port's world data (read by Claude): within 47 m of this spot there is NO turret or pad instance (nearest `com_weap_gunturret104` at (57.0, 51.4, 85.1), 47 m) and no building instance within 28 m (pillars, 'gutter' sound emitters). The deck's own mesh is not one of the manifest's instances. KAM1 has 33 `tur_*` instances (21 `tur_weap_built_gunturret` build pads, 12 `tur_bldg_chaingun_roof`) - the RE note counts 16 pads on KAM1 (`re-build-zones.md`, RespawnTime 120 s).
- So two readings, to be told apart from the PS2: (a) the PS2 has a pad or turret on these decks and the port's world lacks the instance (a bake that drops it, or an object the manifest does not list); (b) the PS2's turrets here are pads the PS2's bots BUILD within seconds (card TBLD) and the port neither builds nor draws them. The census (acceptance 1) decides: PCSX2 frame of this deck, from this spot, beside the port's. There is no PS2 reference frame within 80 m of it (`pcsx2/ref/bf2_kam1`: the nearest eye is 83 m).

- 2026-10-09 12:45 planner. GRFL (296fa0c) landed the grenade fix and measured it at the maintainer's SECOND deck only (103.1, 52.4, 75.0: 0 of 4 through). His first deck (the F9 at tick 10701, `playtest/prev1.json`, y about 70 - this card's platform) was not measured, and 935 of Kamino's 937 remaining pass-throughs are throws that cross `kam_bldg_mushplatform1`'s soldier-only skirt. When this card stands at the first deck: run `ordnance_census`'s four grenades at his position from the snapshot and write the count here and on GRFG.

## Findings (2026-10-22 06:50, capture, lane 2; main 9193a4c) - the port already matches the PS2: nothing to fix
Full write-up: `engine/artifacts/lead_tmp/tslg/FINDINGS_TSLG.md` (F68, F69; the guard keeps `pcsx2/ref/FINDINGS.md` closed to this session).
- **The maintainer's platforms are the far ends of `kam_bldg_aaa_bridge` walkways** (his first F9 = `kam_bldg_aaa_bridge5`, his second = `kam_bldg_aaa_bridge1`; 7 on KAM1). A plain prop; its disc model has two bones (root, collision) - no hardpoint for a turret. **The PS2 has no turret on them either**: PS2 frames at both spots (his own camera pose for the second) beside the port's at the PS2's exact poses, `lead_tmp/tslg/pairs_playtest.png`, `pairs_playtest2.png`.
- **The turret "at the base of this leg thing" is where the PS2 has it**: `tur_weap_built_gunturret1013` (-102.0, 69.35, 142.34), on the landing platform the bridge grows from; the PS2 holds a built turret at exactly that point in both eras.
- **Acceptance 1 (census)**: KAM1 conquest = 21 `tur_weap_built_gunturret`, all BUILT from the load on the PS2 (state 0, 800/800, kam1g_con and kam1c_con RAM), each at the port's position to 0.000 m; 42 PS2 | port pairs (two views each) show every one on its deck, same place, model and facing: `pairs_t1..3.png`, table `instance_table.md`. The other 12 of the 33 (`tur_bldg_chaingun_roof`) are the `kamino1_campaign` layer only - in no conquest battle on either side.
- **Acceptance 2 (other maps)**: RAM census of every armed building, all 17 BF2 maps: PS2 122 / port 122 / matched 122, max 0.000 m, 0.00 deg; the 48 build zones = exactly the PS2's unbuilt pads. BF1's 15 maps too: 167 / 167 / 167, 0.000 m, 0.00 deg (`tools/census_bf2.py`, `census_bf1.py`; control: merging every layer flags 34 port-only rows). No defect, so nothing to fix.
- **Acceptance 3 (test)**: void - there is no misplacement to pin; the census scripts are the re-runnable check (2 minutes for 32 maps, from the kept no-AI states).
- **Grenades (planner note)**: `ordnance_census` ORDN_AT at his first deck (-125.6, 70.2, 151.0): 0 of 4 through (body `kam_bldg_aaa_bridge5`); second deck 0 of 4 (`kam_bldg_aaa_bridge1`). Written on GRFG too.
- Found on the way (BZONE1's question): every BF1 `com_weap_gunturret` (118 on 11 maps) is UNBUILT at side select on the PS2; the port spawns them built. Note on BZONE1.
- For the maintainer: if you want turrets on the AAA-bridge balconies anyway, that is a deliberate deviation from the PS2 - say so in INBOX and it becomes a card. Default taken: faithful (no turret there).
