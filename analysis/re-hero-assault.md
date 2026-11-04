# RE: Hero Assault on Mos Eisley (`tat2g_eli`, issue #106)

Read from the PS2 BF2 disc scripts (`tat2g_eli.luac`, `objectivetdm.luac`, Lua 5.0 bytecode listed by hand), the ELF
(`extracted/bf2/lvl/SWBFRNT2.ELF`) and its decomp (`SWBFRNT2.ELF-decomp.c`, line numbers below). Tags: [V] read, [D]
derived, [H] hypothesis. Nothing here was compared against the running original game.

## 1. The rules

- [V] Score: `objectivetdm.luac`, `OnCharacterDeath(player, killer)`. No killer or the same team returns; otherwise the
  killer's team gains one point. When the limit is at or below the team's points the objective completes for that team.
- [V] Limit: a celebrity deathmatch reads `ScriptCB_GetAssaultScoreLimit`, `FUN_0019b868`, which reads `DAT_005fa898`.
  Its ELF word is 180, and the session's `iASSScoreLimit` row (range 50..1000) writes it. The port's constant is
  `ASSAULT_SCORE_LIMIT`; the shell passes `session.ass_score`.
- [V] `multiplayerScoreLimit: 100` is set in `tat2g_eli.luac`, and nothing else reads it: no other script and no string
  in the ELF. The issue's "default 100" is this unread field, so the port runs 180.
- [V] No time limit: `GetGameTimeLimit` returns 0 in this objective.
- [V] The bot count comes from `ScriptCB_SetNumBots(ScriptCB_GetASSNumBots())`, the port's `session.ass_numbots`.
- [V] Reinforcements: `SetReinforcementCount` (`FUN_0018c3e8`, line 32113) turns a negative count into 0x7fffffff. The
  generator's existing `NO_TICKET_LIMIT` writes 999999.
- [V] Bleed and domination belong to `ObjectiveConquest`, which this script does not create.

## 2. No hero rules at all

- [V] The drain in `FUN_0034cea8` (line 331703) is gated by `FUN_0034da30` (line 332157), which reads hero bit 33 of
  class+0x330.
- [V] Only `FUN_0036e770` (line 355209) sets that bit, reached through `FUN_0036e800` from `SetHeroClass` (0x18cb18, line
  32395). `tat2g_eli` never calls `SetHeroClass`, and `EnableSPHeroRules` is a stub.
- [D] So the mode has no hero manager: no unlock, no drain and no kill-heal. The port leaves `sim.heroes` as `None`.
- [V] The death clip is the exception. `FUN_002379f0` (line 151136) takes the hero death state 16 when hero bit 33 is
  set or when class+0x778 bit 4 is set, and that bit is `FleeLikeAHero` (setter at line 171022, key 0x268dac2d). This is
  the only read of that bit. `com_jedi_default` and `com_hero_default` author it, so all 17 classes here kneel with
  `stand_deadhero` even with bit 33 clear. Grievous's clip table has no `stand_death_*` rows at all.
- [V] The script also turns hero music and hero VO off (`ScriptCB_EnableHeroMusic(0)`, `ScriptCB_EnableHeroVO(0)`).

## 3. Which class a bot takes

`FUN_0036f198` (line 355577), called from `FUN_0034ffe0` (line 334010), as also read in `re-ai-class-selection.md`:

- [V] It counts the class assigned to each member of the team. The reserve holds while a human is on the team or the
  game clock is under 60 s.
- [V] It shuffles the classes with the game LCG. The first pass takes a class under its minimum, the second a class
  under its maximum; while the reserve holds, a class one below its maximum is refused in both. Otherwise it returns
  -1 and the member waits.
- [V] A member keeps its old class counted until `FUN_0034f1f8` (lines 333346, 333357) clears it on a -1, so the port
  tries with its own class counted and then without.
- [D] With the script's min 1, max 2, the human's team fields at most 9 hero classes. The bot team fields 8 for the
  first minute, then up to 12.

Measured in the port (seed 42, 60 s): 9 and 8 distinct classes alive at once, points [15, 8]. Run to its end, the round
closes at 180 about 10 minutes in, with team 2 ahead. Those are port runs, not measurements of the original.

## 4. World

- [V] The `tat2_tdm` layer holds six `com_bldg_controlzone_ctf` posts: `eli_cp1`, `eli_cp7`, `eli_cp2` (team 1) and
  `eli_cp8`, `eli_cp6`, `eli_cp3` (team 2).
- [V] None names a capture region, and `eli_cp8` names no control region either. The layout normally skips such an
  instance, so the mode keeps it (`Manifest::spawn_only_posts`). Conquest is unaffected, because `yavin1_commondesign`
  holds regionless posts the port does not field today.
- [V] A null capture region means nobody counts (`FUN_00359b78`; `re-command-walker-post.md` section 3), so these posts
  cannot be taken. The port stops counting occupants for the whole mode.
- [V] The VO and music lines give team 1 the Alliance's and team 2 the Empire's, hence `hero` as `all` and `villain` as
  `imp` in the generated mission.

## 5. Not measured

- PROVISIONAL, trace: the PS2 Deathmatch AI goal. Bots use the existing `retarget_hunt` path, and the layer's base nav
  graph was only seen working in headless runs.
- PROVISIONAL, replace: the DLC heroes' min 1, max 2 in `heroes.rs` `assault_dlc_class` has no disc evidence. The DLC
  path did not run here, because `side_bf2/dlc_heroes.ron` is not installed.
- trace: `cor1g_eli` exists nowhere on this machine and in no correction table. The generator writes it only when a
  player's own Lua holds it, and the menu offers it only with a mission. `game/src/dlc.rs` and PLAN say the DLC is only
  the two heroes, so the Coruscant script may not exist. Coruscant is unverified.
- measure: the original's round length and score pace against the port's.
