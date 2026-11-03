---
id: ANNC3
title: The music follows the listener's FACTION too (the mission's music calls are keyed by the script's team number, as the commander's lines were before ANNC2: playing the other side or another era gives the other army's music), BF1's no-post line, and the HUD's bleed flash on BF2's script rate
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Filed by ANNC2's build (2026-10-05, report reports/20261005_1045_card_ANNC2.md). ANNC2 keyed the commander's lines by the armies really in the battle; three neighbours were left.
1. **Music.** game/src/music.rs bind_with maps the script's team n to port team n - 1 for the ambient, victory and defeat calls (line 51). The shell can seat any army in either slot, so a Republic player on a GCW mission (the maintainer's KAM1 playtest of 2026-10-04 was that pairing) gets the Rebels' music, and the other side of a mission gets the enemy's victory / defeat cue. Same rule as game::announce::battle_lines: the team of the listener's faction; an army the mission lacks takes the Music of the same role with its own prefix when the bank holds it (check the names first - mus_ records may not carry a faction prefix; then keep the slot's).
2. **BF1's no-post line** (re-announcer.md T7; B1 FUN_002ad830 238807-238824): when team 1 holds no post the low-reinforcement line [listener][team 1] is asked again on the first frame of the 20 s defeat countdown, no latch. Not built.
3. **The HUD's bleed flash** (main/battle.rs HudModel.bleed) reads the sim's clamped bleed_per_sec; on BF2 the manager reads the script's rate (Commander::script_bleed). What the PS2's HUD reads was not checked - RE that read (BHUD2's source) before changing it.
## Evidence
- Music: engine/crates/game/src/music.rs 51-88; missions/*.ron music blocks (Lua team numbers).
- T7: analysis/re-announcer.md section 0 row T7 and section 6.
## Acceptance
1. A unit test like announce::tests::factions for the music table: mission (cis, rep), walk [rep, cis], listener 0 - the ambient, victory and defeat entries are the Republic's.
2. MUSIC_LOG over a round of bf2_kam1_con with the armies rep,cis and of bf1_kas2_con_cw with imp,all: no Music of an army that is not in the battle (count pasted, before / after).
3. T7: a unit test - BF1 rules, team 1 loses its last post: the low line is asked on that frame, and again at the next loss.
4. HUD: the RE answer in the report; a change only with a cited read.
declares: nothing (presentation).
## Notes
- 2026-10-05 (audit of ANNC2): two neighbours filed as their own cards. ERAM - the shell starts `<map>_con.ron` for every pairing, so the music calls this card re-keys are the GCW script's even for Republic against CIS; land ERAM first or together. SNDE - on BF1 Bespin Platforms CW the `cis_bes_amb_*` / `rep_bes_amb_*` Music stream properties resolve to Cloud City's sub-level (11 names, `lead_tmp/audit-annc2/sublevel_census_bf1.txt`): check a Music take's stream against the mission's own sub-level before blaming the faction key.
- 2026-10-06 planner: both neighbours this card waited for have landed - ERAM (8d6c15a: the shell loads `<map>_con_cw.ron` for a Clone Wars pair and seats the player on his army's team; `game::mission::pick`) and SNDE (e45d75a: a Music's stream property resolves in the mission's own sub-level, which is what fixed Bespin Platforms CW's music takes). Re-read item 1 against `WalkSim::with_teams_seat` before building: the player can now sit on team 1, so "script team n = port team n - 1" may already hold for a mission's own pair and the open case is the cross-era pair.
- 2026-10-06 planner: **item 5, from MUSV's report (no card held it after MUSV closed): BF1's music state rule is still PROVISIONAL** (`core/src/music.rs` `state_bf1`; `analysis/re-ambience-music.md` section 11). Check it live on the PS2 with `engine/artifacts/lead_tmp/ambm/tools/ram_music.py` (the manager's state word against the reinforcement counts over one BF1 round) and drop the PROVISIONAL label or fix the thresholds. Acceptance: the RAM log's state changes against the port's `MUSIC_LOG` for the same ticket counts, pasted. If the PCSX2 session does not fit this card's session, split it off as a `measure` card rather than dropping it.
- 2026-10-06 planner: MUSV also found that a windowed battle and a headless one cannot be compared tick for tick (an idle human sits in a slot: AI 24 / 25 against 25 / 25). Acceptance 2's MUSIC_LOG runs are headless; do not ask for "the same game times" from a window.
- 2026-10-21 build (lane 1, report `reports/20261021_1955_card_ANNC3.md`): branch annc3 c007fd3, all four items built and
  checked. (1) `music::tests::factions` passes. (2) Bake census, 57 missions x 12 pairs: foreign-army entries 3420 -> 320
  of 6840 (left only on single-era banks); headless MUSIC_LOG + `MUSIC_ARMIES=rep,cis` on bf2_kam1 / bf1_kas2: 36 of 36 ->
  0 of 37, sim hashes unchanged. (3) T7: `bf1s_no_post_line_repeats_at_every_loss`. (4) HUD: BF2 reads the script's
  rate (verifier 6/6). 8 mutations caught. PARKED: the RTX was lost mid-gates (gpu moved 97 goldens on the iGPU); every
  other gate PASS. Item 5 split off as card MUSB1.
