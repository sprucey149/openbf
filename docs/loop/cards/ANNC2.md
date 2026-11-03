---
id: ANNC2
title: The commander hears the right inputs: his bleeding / reinforcement lines follow the listener's FACTION (playing a mission's other side now gives the enemy's commander), and on BF2 maps the last stand speaks as the PS2's does (the script's 3 s bleed rate at every post event: 77 lines missing in 23 of 31 battles)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Filed by the audit of ANNC (2026-10-02, PARTLY; `reports/20261002_0135_audit_ANNC.md`, evidence
`engine/artifacts/lead_tmp/audit-annc/README.md`). The commander's RULES are the PS2's; two of his INPUTS are not.

1. **Playing the mission's other side gives the wrong commander.** The shell seats the player on team 1's slot and has
   no side selector, so playing e.g. the Republic on Utapau (the script's team 2) means swapping the two factions. The
   bleeding and low-reinforcement lines stay keyed by the script's team number (`game::announce::battle_lines`), the
   post lines follow the faction: a Republic player hears `cis_off_com_report_us_overwhelmed` and `cis_off_victory_im`
   beside `rep_off_com_report_*_commandpost`.
2. **BF2 maps: the last stand is silent.** The PS2's manager reads the rate the conquest script hands to
   `SetBleedRate` (team+0x2c): set at Start and at every finished capture / neutralisation to the UNCLAMPED rule, and
   zeroed by the script's bleed timer at its first elapse (3.0 s) that finds alive >= reinforcements — not set again
   until the next post event. So once a team is down to its last units, every post event starts a new 3 s bleed and
   the bleeding line is said again. The port feeds the announcer `bleed_per_sec`, which `cp::apply_bleed` holds at 0
   for the whole last stand. BHUD2's HUD model reads the same field.

## Evidence
- Factions: `runs/bf2_uta1_con_cw.ron.swap.log` (3 of 14 lines from the enemy's commander), `bf2_myg1_con.ron.swap.log`
  (9 of 15). Code: `ui/src/shell.rs` rows TeamA / TeamB, `main/windowed.rs` `start_battle`, `walk.rs` (the player is
  always a team-0 unit), `announce.rs` `battle_lines` / `for_battle`.
- Last stand, PS2 [V]: `objectiveconquest.luac` functions 7.0 (`SetBleedRate(team, rate)` + timer 1.0 at `rate`),
  7.3.0 (`alive < reinforcements` -> -1 ticket, +1.0; else `SetBleedRate(team, 0.0)`; `StopTimer`), 7.2 (called at
  Start and from the capture / neutralise / post kill / post respawn handlers, under `multiplayerRules`, which every
  `*_con` script sets) — `python3 lead_tmp/audit-annc/dis50.py <luac> 7.3.0`; `SetBleedRate` -> `FUN_0036ea18` ->
  `team+0x2c` (B2 367639), read by the manager `FUN_00373b80` (B2 371155).
- Last stand, PS2 [L]: `lead_tmp/audit-annc/live1.csv` — BF2 Utapau CW as CIS, the Republic's tickets held at 3. Five
  post events with CIS on 4+ of 6 posts (game s 71.60, 85.42, 91.69, 135.47, 158.56): each sets `team2.rate` 0 ->
  0.3333 in the owner change's own sample and plays `cis_off_com_report_enemy_losing` (0.07 s later on an idle voice,
  2.6-3.6 s behind the capture line); the rate is 0 again 3.003 s later, four times of four. The three events with
  CIS on 2-3 posts leave it 0.
- Port: over 31 BF2 missions (seed 42, whole AI rounds through `Commander::frame`; two Kamino runs hit the tick cap)
  the port asks 133 bleeding lines where the PS2's rate asks 210: **77 missing in 23 of 31 battles**, none extra, every
  other line equal (`python3 lead_tmp/audit-annc/cmp_bleed.py keep`; all 77 at alive >= tickets, at a post event).
  BF1 discs have no such script: BF1's engine gates its rate on alive < tickets every update (B1 238700-238702), which
  is what the port does — leave BF1 maps as they are.

## Notes
- Not built by ANNC and on no other open card — do them here or split: (a) BF1's no-post line (B1 `FUN_002ad830`
  238807-238824: when team 1 holds no post, the low-reinforcement line `[listener][team 1]` is asked again on the
  first frame of the 20 s defeat countdown, no latch); (b) the posts' `Info` entries and the destroyable posts' lines
  (BF1 17 Info + 8 enemy-Lost properties on technounion / shield generator / bunker instances; BF2 Hoth's shield
  generator 4 + 2) — they need the destroyable posts first.
- NOT a gap: `common_objComplete` (re-announcer.md T8). `objective.luac` function 9 broadcasts it only
  `if not self.multiplayerRules`, and all 33 BF2 `*_con` scripts set `multiplayerRules = true`.
- `core/src/announcer.rs` 82-84 and the `cp_lines` test say no retail post sets `VO_<L>_<enemy>Lost`: 10 entries on
  capture posts do (BF1 bes2 cp5, geo1 cp6, yav1 temple). The code is table-driven and right; fix the comment.
- Splitscreen, when it comes: with two local players on different teams the PS2's listener is the attacking team
  (`SetAttackingTeam` -> mgr+0xf4, `FUN_00373fa0`).
- 2026-10-04 (the maintainer's playtest, via Claude; value 4 -> 5 at his word): "I'm playing as the empire, and it has empire voiceovers and droid voiceover lines. This was also happening when I was playing as the republic, it was sometimes playing rebel lines." The first is this card's listener-faction rule; the second is a WRONG ERA's lines (Republic hearing Rebel, Empire hearing droid), which this card did not name - find where a line's faction is picked (the commander's and the units' own voices, USND / SQACK's squad answers) and add it to the acceptance: over a round on each side of KAS2 and KAM1 no line of a faction that is not in the battle is played.
- 2026-10-05 LANDED cc4219f (report `reports/20261005_1045_card_ANNC2.md`). Acceptance 1-4 run: `factions`,
  `last_stand_rate`, the PS2 pair (5 events, 5 lines, rate 0 after 2.967 s against 3.003 s), census BF2 205 = 205
  (old feed on today's sims: 135, 70 missing in 21 battles), BF1 unchanged (415 lines either feed). The maintainer's wrong-era
  lines were the era-mix case of rule 1 (the Republic in the Rebels' slot of a GCW mission): 8 rounds on KAS2 / KAM1,
  0 of 124 lines from an army not in the battle. Leftovers (music by script team, BF1's no-post line, the HUD's
  bleed read) are card ANNC3; the destroyable posts' lines still wait for destroyable posts.
- 2026-10-05 AUDIT: PARTLY (`audits.md`, report `reports/20261005_1150_audit_ANNC2.md`, evidence `engine/artifacts/lead_tmp/audit-annc2/README.md`). What was built holds: the script's rate against the bytecode and both ELFs, the census reproduced (205 = 205, old feed 135, BF1 415), every line's role, two mutations. Not closed by it: the maintainer's Empire battle on BF1 Kashyyyk Docks was the default pairing, not an era mix, and still plays Clone Wars takes at every post event - the sound bank keeps one definition per name for the whole planet LVL (card SNDE, value 5). The "era mix" itself is mostly the shell starting `<map>_con.ron` for every pairing (card ERAM, value 4). The census's "205 = 205" is the port against a copy of its own rule: it shows wiring only.
