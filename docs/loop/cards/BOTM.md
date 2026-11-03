---
id: BOTM
title: Bot-quality numbers in the sweep, the PS2's beside the port's: jitter (reversals, path / net), standing-next-to-each-other, team kills, wall contact, time in contact - so 'the bots look wrong' is a gate, not the maintainer's eye
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Every bot defect so far was found by the maintainer playing (bowl, walls, tweaking, team kills). The sweep proves rounds END; nothing measures whether a bot MOVES like a soldier. The PS2's own soldiers are the yardstick, and where the maintainer wants better than the PS2, BOTD sets the bar.

## Result (2026-10-02, lane 2) - `analysis/bot-quality.md`
- Instrument: `BOT_SAMPLES` (round_to_outcome, 4 Hz + shots / hits / kills), `scripts/bot_quality.py` (one 4 Hz game-time
  grid for both sides), `sweep_seeds.py --botq` (a table per map in seeds.md, PS2 column from `scripts/bot_quality_ps2.json`).
- Checks: `--selftest` 7/7 (an oscillator given 19 / 9 turns reads 19 / 9, also sampled at an uneven 5.4 Hz); the maintainer's
  snapshot #30 reads 26 reversals (46.1 m path, 3.1 m net); PS2 retarget one per 308 foot-s (CONT 323).
- PS2 captures (live RAM, player spawned; 0.08-0.25 s game time apart, never sparser than 4 Hz): BF2 KAM1 227 s, POL1 160 s,
  DEA1 174 s, BF1 KAS2 165 s; the maintainer's scene (KAM1, player held at (-4, 60.3, 108.6)) 150 s. Data: `lead_tmp/botm/ps2/`.
- Worst gaps: path in a staying 10 s window 6-20 m (PS2 1-9 m); pinball windows 2-8 % (PS2 1-3 %); 20 s spells within 2 m
  0.3-8 % (PS2 8-32 %); a teammate within 1.5 m 12-22 % (PS2 2-9 %); an enemy within 20 m 2-4 % (PS2 15-19 %, KAM1 / DEA1);
  BF2 turret seats 0 % (PS2 9-18 %); team kills 16-26 % of kills (PS2 not measured: FFLN step 1). The maintainer's scene: PS2 bots
  within 20 m 1.98 reversals per 10 s, none over 6; the port's snapshot 9.0, 3 of 6 over 6.
- Not done (handed on): the PS2's team-kill share and fire discipline (FFLN), hubs / connections never visited and
  turret time with no target (BOTD, if it wants them).

## Re-check and re-take after JITR (2026-10-02 21:05, lane 1; main ac605ca) - `lead_tmp/botm/post_jitr/seeds.md`
- Acceptance re-run: `bot_quality.py --selftest` 7/7 (oscillators read 19 / 9, also at an uneven 5.4 Hz); snapshot #30:
  40 points, path 46.1 m, net 3.1 m, reversals 26 PINBALL. The pre-JITR KAM1 table rebuilt from the stored samples
  (path in a staying window 13.3 m, 3.71 reversals, a teammate within 1.5 m 13.4 %).
- `sweep_seeds.py --seeds 1-4 --ticks 16500 --maps bf2_kam1 bf2_pol1 bf2_dea1 bf1_kas2 --botq` on main with JITR in
  (port median of 4 seeds, before JITR -> after | PS2):

| | KAM1 | POL1 | DEA1 | KAS2 |
|---|---|---|---|---|
| path in a staying window (m) | 13.8 -> 8.8 \| 1.3 | 16.1 -> 9.5 \| 9.3 | 6.3 -> 4.1 \| 5.4 | 19.3 -> 20.0 \| 1.3 |
| reversals in a staying window | 3.76 -> 2.96 \| 0.36 | 4.86 -> 2.26 \| 1.07 | 3.52 -> 0.72 \| 0.44 | 4.38 -> 6.47 \| 0.36 |
| pinball windows (%) | 5.9 -> 5.1 \| 1.2 | 8.1 -> 3.2 \| 3.3 | 2.7 -> 0.5 \| 1.5 | 2.1 -> 2.8 \| 1.6 |
| a teammate within 1.5 m (%) | 12.5 -> 8.7 \| 6.6 | 21.9 -> 17.9 \| 5.7 | 21.5 -> 15.1 \| 9.3 | 11.8 -> 10.0 \| 2.0 |
| team kills / kills (%) | 15.6 -> 11.6 | 24.8 -> 21.8 | 25.8 -> 30.0 | 18.4 -> 13.0 |

- JITR moved POL1 and DEA1 to about the PS2's staying numbers; KAM1 is better and still 7-8 x the PS2; **BF1 KAS2 did
  not improve** (reversals in a staying window 6.47, range 5.37-8.23 over the 4 seeds; before 4.38) - for BOTD / SAIR,
  not a new card. Unchanged gaps: an enemy within 20 m (KAM1 2.2 %, DEA1 2.7 % vs 15-19 %), BF2 seats 0 %, 20 s spells.
- Note: only 11 of 16 rounds finished inside 16500 ticks; the tables cover the first 540 s either way.

## The bot programme (the maintainer, 2026-10-02) - five cards, read together
`JITR` (stop the tweaking now) -> `FFLN` (teammates in the line of fire) -> `BOTM` (bot-quality numbers, PS2 beside the port) -> `SAIR` (RE the PS2 foot soldier's own AI) -> `BOTD` (the design: what the best bot for this game is, then the build cards). `OBST` (walking into props) belongs to the same family.
The maintainer: "If the bots suck, the whole game is worthless so we NEED to get this right." He agrees to RE the PS2 soldier AI FIRST, then keep what is good and DELIBERATELY do better where the originals annoy (the originals' bots are not perfect; better difficulty levels, more awareness). The port's soldier brain today is ~9,200 lines (`core/src/ai`, `commander.rs`) of rules added one symptom at a time (hold, shuffle, strafe, dodge, slide, fan, stuck, stall, lost, recycle), most of them OURS/PROVISIONAL: no RE note exists for the soldier's second-to-second movement and combat.
Reference pulled 2026-10-02: the official mod-tools documentation as markdown, `community_mods/gametoast_lua/doc/StarWarsBattlefrontII/documentation/` (`game.md`, `ZeroEditor_guide.md` - barriers, planning hubs/connections/weights/filters, hint nodes; `odf_guide.md`; `Battlefront2_scripting_system.md`), executable strings per platform `community_mods/gametoast_lua/doc/StarWarsBattlefrontII/StringsFromExecutable/` (the PS2's too), a PC DEBUG build `community_mods/gametoast_lua/doc/StarWarsBattlefrontII/Debugger_exes/`, 15 years of Gametoast forum posts `community_mods/gametoast_lua/doc/Archives/Gametoast.com/*.json`.
