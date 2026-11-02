---
id: SAIR
title: RE the PS2 foot soldier's own AI, both games: how it walks a connection, goes round barriers and other units, takes hint nodes, picks a target, fires, strafes, dodges, takes cover and treats teammates - the port's soldier brain is ours, not the PS2's
milestone: M3
value: 5
phase: re
status: done
deps:
files: analysis/
declares:
needs:
effort:
branch:
resume:
created: 2026-10-02
alpha: yes
first: 4
---

## Why
There is an RE note for the goal (re-conquest-goal), difficulty, snipers, walkers, squad orders, grenades - and none for the thing the maintainer watches: a soldier's second-to-second movement and combat. The port invented it. The maintainer, 2026-10-02: RE the originals first, then analyse where THEY are flawed and do better there.

## Deliverable
`analysis/re-soldier-ai.md` with [O]/[V] marks, and its last section a three-column table **PS2 does | the port does | difference** for every item above - this table is BOTD's input. Plus a plain list "what in the PS2's bot is weak or annoying by its own code" (stands in the open, no flanking, ignores friends, etc.) - facts only; BOTD decides.

## The bot programme (the maintainer, 2026-10-02) - five cards, read together
`JITR` (stop the tweaking now) -> `FFLN` (teammates in the line of fire) -> `BOTM` (bot-quality numbers, PS2 beside the port) -> `SAIR` (RE the PS2 foot soldier's own AI) -> `BOTD` (the design: what the best bot for this game is, then the build cards). `OBST` (walking into props) belongs to the same family.
The maintainer: "If the bots suck, the whole game is worthless so we NEED to get this right." He agrees to RE the PS2 soldier AI FIRST, then keep what is good and DELIBERATELY do better where the originals annoy (the originals' bots are not perfect; better difficulty levels, more awareness). The port's soldier brain today is ~9,200 lines (`core/src/ai`, `commander.rs`) of rules added one symptom at a time (hold, shuffle, strafe, dodge, slide, fan, stuck, stall, lost, recycle), most of them OURS/PROVISIONAL: no RE note exists for the soldier's second-to-second movement and combat.
Reference pulled 2026-10-02: the official mod-tools documentation as markdown, `community_mods/gametoast_lua/doc/StarWarsBattlefrontII/documentation/` (`game.md`, `ZeroEditor_guide.md` - barriers, planning hubs/connections/weights/filters, hint nodes; `odf_guide.md`; `Battlefront2_scripting_system.md`), executable strings per platform `community_mods/gametoast_lua/doc/StarWarsBattlefrontII/StringsFromExecutable/` (the PS2's too), a PC DEBUG build `community_mods/gametoast_lua/doc/StarWarsBattlefrontII/Debugger_exes/`, 15 years of Gametoast forum posts `community_mods/gametoast_lua/doc/Archives/Gametoast.com/*.json`.

## Folded in from FGHT (planner 2026-10-02 17:45) - the fight order is one of SAIR's questions
FGHT's RE half is SAIR's item 3; one RE pass, not two. Answer it with FGHT's leads and numbers:
- BF2's fight order: `FUN_002c3730` (message -> order), `FUN_002c31e0`, `FUN_002c4c80` (the fight behaviour, order type 2, 0x250 bytes), the type-0 order after damage `FUN_002c28e8`; BF1's twin. Does the bot move toward its target, to what range, for how long, when does it give the target up, where does the capture order resume?
- PS2 measure: `lead_tmp/cont/cont_ps2.py` logs order type, behaviour state and position at 4 Hz - distance to the fight target at the order's start and end, duration, displacement. PS2 bots spend 20-28 % of alive time in that order, 16-19 % at hint nodes, 5-12 % entering / manning turrets (`analysis/re-conquest-goal.md` section 5).
- The gap it explains: an enemy within 20 m 3.0 % of alive unit-seconds on BF2 KAM1 (PS2 17.0 / 24.9 %), POL1 23.1 % (PS2 31.9 %), with kills / min already the PS2's (17.3). FGHT is now a build card behind SAIR and BOTD (its acceptance numbers are a candidate BOTD bar).

## Done 2026-10-02 (lane 2, docs commit e98b20e; report `reports/20261002_2245_card_SAIR.md`)
`analysis/re-soldier-ai.md`: section 0 the one-table answer, 12 the PS2 | port | difference table (BOTD's input), 13
the PS2's own weak points, 11 the eight replays against live RAM with residuals (five for BF2, two for BF1 and the
stall census), 14 the checks and the verifier (two passes, 84 claims: 50 SUPPORTED, 33 PARTLY folded in, 1 NOT
SUPPORTED and replaced). The maintainer's three: 10a turrets, 10b dead areas (the unused hubs per map), 10c BF1's stops.
Cards: new HUBP (value 4) and ENGR (value 3), both behind BOTD; notes on FGHT, GRAI, JETP. No engine code.
Not opened (the note's section 14 lists them): why each failing walk to a turret fails, BF1's repair / snipe / mine
helper bodies, the render-mark term of the level of detail, Coruscant (no capture).
