---
id: MSND
title: Mission data still unconsumed: `spawn_delay`/`spawn_interval` into `SpawnWaves`, `attacking_team`, the per-team alive-AI cap on BF1 kas1/kas2/nab2, the locals teams
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## The maintainer's decision (2026-09-25)
The locals teams do what the PS2 does (infinite tickets per `analysis/re-tickets-spawn.md` §2.1). No design call needed.

## Why
The mission RONs carry authored round rules the sim ignores; the asymmetric BF1 maps play with the wrong unit counts.

## Evidence
analysis/PLAN_LOG_2026.md §ARCHIVED 2026-09-25 HANDOFF-OPUS.md HOLD 16; `analysis/re-tickets-spawn.md` §2.1 (a locals team's infinite tickets). What a locals team DOES in the sim is a
design call — the maintainer (STATUS For the maintainer); consume the rest first.

## Acceptance
Each field consumed with a test; the sweep / arena moves declared (a re-baseline) and explained per map.

## Notes
- UAUD1 (2026-09-29, analysis/unit-audit-bf1.md §2): the RON's `locals` keeps ONE (class, count); the disc's team 3 has 2-3 classes on END1
  (ewk trooper 4 + repair 6), KAS1/KAS2 (wok mechanic 2, rocketeer 2, warrior 3), NAB1 (gun soldier 5, rider 2), TAT1
  (tusken raider 5, hunter 2); GEO1 has the Geonosian 7; HOT1's "locals" is team 4, one `imp_droid_probe`. The PS2 has
  them live (RAM census). None of their models is baked (`ewk_inf`, `wok_inf_wookie`, `nab_inf_gungan`, `tat_inf_tusken`,
  `gen_inf_geonosian`); several have OverrideTexture and SkeletonRootScale (USKN) and NoEnterVehicles 1.
- UAUD2 (2026-09-29, analysis/unit-audit-bf2.md §2): BF2 locals the RON trims to one class: END1 `ewk_inf_trooper` 3 + `ewk_inf_repair` 3
  (units 6; `ewk_inf_scout` is loaded but not added), KAS2 `wok_inf_warrior` 2 + `wok_inf_rocketeer` 2 +
  `wok_inf_mechanic` 1 (units 5). GEO1 (Geonosian 7) and TAT2 (Jawa 7) have one class. TAT3 (not a port map, B2JP):
  `gam_inf_gamorreanguard` 3 of 2 units, melee axe.

## Planner note (2026-09-30, M-ALPHA)
`alpha: yes` — the maintainer's alpha list. Value 3 → 4. Showcase maps: BF1 NAB1, BF1 TAT1 (the port's `tat1`), BF1 YAV2, BF2 UTA1, BF2 MYG1, BF2 HOT1 (PLAN §Milestones M-ALPHA).

- Planner 2026-09-30 17:30: ANNC (M3 5, alpha) owns the commander voice-overs (cis_off_com_report_*, SetOutOfBoundsVoiceOver, REP_bonus_*); drop them from this card's scope if ANNC lands first, and vice versa.

- ANNC RE 2026-09-30 (`analysis/re-announcer.md`): (1) the commander lines are ANNC's now (build): its mission-RON fields carry SetBleedingVoiceOver / SetLowReinforcementsVoiceOver / SetOutOfBoundsVoiceOver per mission — drop them from this card. (2) **BF1's `AddBleedThreshold` tables are LIVE on the PS2** (§5: the team's vtable slot +0xc qsorts them ascending; live RAM `[(1,3.0),(11,2.25),(21,0.75),(31,0.0)]`, drain rate/4 = 0.75 / 0.5625 / 0.1875 tickets/s at no / one / two posts, both teams can bleed at once). `re-tickets-spawn.md` §2.4 said "inert" and is corrected. The port plays BF2's bleed on every map (PLAN §Conquest): no change unless the maintainer wants BF1 maps to bleed BF1's way — a call for STATUS "Calls you may want to reverse".
- 2026-10-16 (card UTALK): the natives' voice is built and waits on the locals team: a fielded Gungan says its
  `gungan_chatter_heartbeat` (LowHealthThreshold 1.1) whenever its own line is over - on the PS2 16 of NAB1's 32 voice lines a
  minute. With the locals fielded, re-run UTALK's census (`lead_tmp/utalk/census/`, tab.py's `lowhealth` row) on NAB1.

## Part 1 (2026-10-22, lane 1, branch msnd 8b30501) - built, parked on the lost RTX
- `MissionDef::apply_round_rules` (game + sweep): spawn waves from `spawn_delay`/`spawn_interval` (all 57 RONs say
  10 / 0.25 = the old fixed default, so sim/sweep/arena/leak/fidelity are identical); `SetAttackingTeam` on
  `Battle.attacking_team` (port team), read by `core::announcer::listener_team` (re-announcer.md 2.0: player 0's team,
  1 with none, the attacking team once a second local player is on the other team).
- Per-team cap on the asymmetric BF1 maps was already right (`initial_ai`: kas1 31 + 25, nab2 21 + 25); pinned by a test.
- Acceptance run: `round_rules_reach_the_sim` + `listener_is_player_zero_or_the_attacking_team` pass; 3 mutations
  (no checked_sub, delay fixed at 10, split ignored) each fail them.
