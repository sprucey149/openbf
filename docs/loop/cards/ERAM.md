---
id: ERAM
title: The shell starts the other era's mission: <map>_con.ron whatever armies are picked, so Republic against CIS on any of the 25 two-era maps runs the GCW script with other rosters (lines the PS2's Clone Wars script never sets, other unit counts / barriers / attackers, the other era's music) and the 25 <map>_con_cw.ron missions are never loaded - load the mission whose pair was picked, and seat the player's army on its own team
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/game/src/main/windowed.rs,engine/crates/ui/src/shell.rs,engine/crates/game/src/mission.rs,engine/crates/game/src/main/battle.rs,engine/crates/game/src/blackbox.rs
declares:
needs:
effort:
branch: eram
resume:
created: 2026-10-05
alpha: yes
domain: behaviour
---
## Why
Filed by the audit of ANNC2 (2026-10-05, PARTLY; `reports/20261005_1150_audit_ANNC2.md`, evidence
`engine/artifacts/lead_tmp/audit-annc2/` = A/). The shell starts `<map>_con.ron` whatever armies are picked
(`ui::shell::navigate` has no arm for the Mode row, `main/windowed.rs` `start_battle` joins
`{name}_{shell.mode}.ron` with mode "con"). On the 25 two-era maps that file is the GCW mission (BF1 Geonosis /
Kamino and BF2 Geonosis are Clone Wars only). So Republic against CIS - a TRADITIONAL pairing, with its own
mission on the disc and its own `<map>_con_cw.ron` in `engine/missions/` - runs the GCW script with other rosters.
The 25 `_con_cw.ron` files are generated, validated and tested, and no battle the maintainer starts loads one. ANNC2 treats
that pairing as an "era mix" and re-prefixes the GCW script's lines.

PLAN's locked feature set: "Era mix-and-match: traditional matchups by default, option for cross-era battles".

## Evidence
- What differs between a map's two missions (`python3 A/mission_census.py`, `A/mission_census.txt`, the 25 RON
  pairs): the number of commander lines on 14 maps, unit counts on 3 (BF1 NAB2, BF2 COR1, KAS2), barriers on 3 (BF2
  KAM1, NAB2, YAV1), the attacking team on 7, and on every map the music calls and the class lists.
- Lines the PS2 never says, asked today (`A/runs/mix_rep_cis/*.log`, the shell's own mission with Republic against
  CIS, seed 42): BF1 KAS2 asks `rep_off_defeat_im`, BF1 YAV2 `rep_off_victory_im`; the disc's Clone Wars scripts set
  no low-reinforcement line - `SetLowReinforcementsVoiceOver` is in 0 of the 21 BF1 CW scripts (1 in each of the 23
  GCW ones) and absent from BF2's `fel1c_con` / `kas2c_con`. BF2 YAV1 asks 15 bleeding lines and
  `rep_off_defeat_im`; `yav1c_con` sets neither kind.
- The other way round, the disc's own slips would come back with the CW missions: `kam1c_con` and `mus1c_con` give
  the CIS listener `cis_off_defeat_im` for the ENEMY's low count and `cis_off_victory_im` for its own (the 4 setters
  off the pattern in `A/mission_census.txt`; the two `low MISFIT` rows of `python3 A/roles.py new swap`).
- No side selector either: the player's army always takes the script's team 1 slot (`walk.rs`: the player is a
  team-0 unit), so "the Republic on Utapau" means the Republic at the CIS's posts.

## Notes
- ANNC3 (music by the listener's faction) reads the same script team numbers: land this first or together, the
  music calls differ per era on every map.
- With SNDE (the era's own sound sub-level) this is the whole of "the battle is the era's own".
- 2026-10-05 planner: value 4 -> 5: every Republic-against-CIS battle the maintainer starts from the shell runs the GCW script, with his army at the other side's posts (25 maps). Undo: `loopctl.py set ERAM value=4`.
